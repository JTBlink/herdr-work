"""Prune generated build caches and archives; never traverse user profiles or plugin data."""
import argparse
from contextlib import contextmanager
import hashlib
import os
from pathlib import Path
import re
import shutil
import time

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MAX_AGE_DAYS = 7


@contextmanager
def directory_lock(path):
    """Nonblocking file lock, also interoperable with Cargo's profile lock."""
    path = Path(path)
    if path.is_symlink():
        raise ValueError(f'Refusing symlink lock: {path}')
    with path.open('a+b') as stream:
        if os.name == 'nt':
            import msvcrt
            if path.stat().st_size == 0:
                stream.write(b'\0')
                stream.flush()
            stream.seek(0)
            try:
                msvcrt.locking(stream.fileno(), msvcrt.LK_NBLCK, 1)
            except OSError as error:
                raise BlockingIOError(f'Directory is busy: {path.parent}') from error
            try:
                yield
            finally:
                stream.seek(0)
                msvcrt.locking(stream.fileno(), msvcrt.LK_UNLCK, 1)
        else:
            import fcntl
            fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
            try:
                yield
            finally:
                fcntl.flock(stream, fcntl.LOCK_UN)


def local_path(path, root):
    """Do not follow a linked cache directory, including links in parent components."""
    root = Path(root).absolute()
    path = Path(path).absolute()
    if not path.is_relative_to(root):
        return False
    for part in [path, *path.parents]:
        if part.is_symlink():
            return False
        if part == root:
            return True
    return False


def tree_stats(path):
    """Newest write and logical bytes, without following symlinks."""
    stat = path.lstat()
    newest, size = stat.st_mtime, stat.st_size if path.is_file() else 0
    if path.is_dir():
        for base, dirs, files in os.walk(path, followlinks=False):
            for name in [*dirs, *files]:
                item = Path(base) / name
                info = item.lstat()
                newest = max(newest, info.st_mtime)
                if item.is_file() and not item.is_symlink():
                    size += info.st_size
    return newest, size


def remove(path, apply):
    _, size = tree_stats(path)
    if apply:
        if path.is_dir():
            shutil.rmtree(path)
        else:
            path.unlink()
    return {'path': str(path), 'bytes': size, 'removed': apply}


def clean_caches(root=ROOT, max_age_days=DEFAULT_MAX_AGE_DAYS, *, apply=False):
    if max_age_days < 0:
        raise ValueError('Cache age must be nonnegative')
    root = Path(root).absolute()
    cutoff = time.time() - max_age_days * 86400
    result = {'items': [], 'busy': []}
    target = root / 'target'
    profiles = [target / name for name in ('debug', 'release')]
    # Only Cargo target triples, never arbitrary nested directories or external target-dir caches.
    if local_path(target, root) and target.is_dir():
        for triple in target.iterdir():
            if re.fullmatch(r'[a-z0-9_]+-[a-z0-9_-]+', triple.name):
                profiles.extend(triple / name for name in ('debug', 'release'))
    for profile in profiles:
        cache = profile / 'incremental'
        if not local_path(cache, root) or not cache.is_dir():
            continue
        try:
            with directory_lock(profile / '.cargo-lock'):
                for entry in cache.iterdir():
                    if not entry.is_symlink() and tree_stats(entry)[0] < cutoff:
                        result['items'].append(remove(entry, apply))
        except BlockingIOError:
            result['busy'].append(str(profile))
    for relative in ('scripts/__pycache__', 'scripts/tests/__pycache__'):
        cache = root / relative
        if local_path(cache, root) and cache.is_dir():
            for entry in cache.glob('*.pyc'):
                if entry.is_file() and not entry.is_symlink() and entry.stat().st_mtime < cutoff:
                    result['items'].append(remove(entry, apply))
    return result


def prune_archives(output, target, *, keep=1, apply=False, protect=None):
    """Caller holds the output lock. Only checksum-owned, same-platform bundles qualify."""
    if keep < 1:
        raise ValueError('At least one archive must be retained')
    output = Path(output)
    suffix = r'\.zip' if 'windows' in target else r'\.tar\.gz'
    pattern = re.compile(r'herdr-work-(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?-'
                         + re.escape(target) + suffix)
    candidates = []
    for archive in output.iterdir():
        checksum = archive.with_name(archive.name + '.sha256')
        if not pattern.fullmatch(archive.name) or archive.is_symlink() or not archive.is_file():
            continue
        if checksum.is_symlink() or not checksum.is_file():
            continue
        try:
            parts = checksum.read_text().split()
            with archive.open('rb') as stream:
                sha = hashlib.file_digest(stream, 'sha256').hexdigest()
        except (OSError, UnicodeError):
            # An unreadable or foreign checksum does not establish ownership.
            continue
        if parts == [sha, archive.name]:
            candidates.append(archive)
    candidates.sort(key=lambda p: (p == protect, p.stat().st_mtime_ns, p.name), reverse=True)
    result = []
    for archive in candidates[keep:]:
        # Drop checksum first: interrupted pruning cannot leave a checksum claiming a missing archive.
        for path in (archive.with_name(archive.name + '.sha256'), archive):
            result.append(remove(path, apply))
    for stage in output.glob('.herdr-work-package-*'):
        if stage.is_dir() and not stage.is_symlink() and tree_stats(stage)[0] < time.time() - 7 * 86400:
            result.append(remove(stage, apply))
    return result


if __name__ == '__main__':
    import json
    from build import TARGETS
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--apply', action='store_true', help='Delete candidates; default only previews')
    parser.add_argument('--max-age-days', type=float, default=DEFAULT_MAX_AGE_DAYS)
    parser.add_argument('--keep-archives', type=int, default=1)
    args = parser.parse_args()
    if args.max_age_days < 0 or args.keep_archives < 1:
        parser.error('Age must be nonnegative and archive count must be positive')
    result = clean_caches(max_age_days=args.max_age_days, apply=args.apply)
    output = ROOT / 'dist'
    if local_path(output, ROOT) and output.is_dir():
        try:
            with directory_lock(output / '.herdr-work-package.lock'):
                for target in TARGETS:
                    result['items'].extend(prune_archives(output, target, keep=args.keep_archives, apply=args.apply))
        except BlockingIOError:
            result['busy'].append(str(output))
    result['bytes'] = sum(item['bytes'] for item in result['items'])
    print(json.dumps(result, indent=2))
