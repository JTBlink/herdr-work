"""Create and verify platform archives, original licenses, provenance and SHA256."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import tarfile
import tempfile
import zipfile
from build import ROOT, TARGETS, metadata, target_dir
from cleanup import directory_lock, prune_archives


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def package(target, output, tag=None, *, keep_archives=1):
    if keep_archives < 1:
        raise ValueError('At least one archive must be retained')
    output = Path(output).absolute()
    output.mkdir(parents=True, exist_ok=True)
    with directory_lock(output / '.herdr-work-package.lock'):
        archive = create_archive(target, output, tag)
        removed = prune_archives(output, target, keep=keep_archives, apply=True, protect=archive)
        if removed:
            print('Archive cleanup: ' + json.dumps(removed), file=sys.stderr)
        return archive


def publish_archive(candidate, output):
    """Preserve the last valid pair if publishing the new checksum fails."""
    paths = [output / candidate.name, output / (candidate.name + '.sha256')]
    backups = []
    for index, path in enumerate(paths):
        if path.is_symlink():
            raise ValueError(f'Refusing symlink archive destination: {path}')
        backup = candidate.parent / f'previous-{index}'
        if path.exists():
            shutil.copy2(path, backup)
            backups.append(backup)
        else:
            backups.append(None)
    changed = []
    try:
        for path in paths:
            os.replace(candidate.parent / path.name, path)
            changed.append(path)
    except BaseException:
        for path, backup in zip(paths, backups):
            if path in changed:
                if backup is None:
                    path.unlink(missing_ok=True)
                else:
                    os.replace(backup, path)
        raise
    return paths[0]


def create_archive(target, output, tag=None):
    info = metadata(target, tag)
    suffix = '.exe' if 'windows' in target else ''
    binaries = {'herdr-work': target_dir(ROOT) / target / 'release' / f'herdr-work{suffix}'}
    if info['projects_backend']:
        binaries['herdr-work-projects'] = target_dir(ROOT) / target / 'release' / 'herdr-work-projects'
    for path in binaries.values():
        if not path.is_file() or path.stat().st_size == 0:
            raise ValueError(f'Compiled binary missing: {path}')
    name = f'herdr-work-{info["version"]}-{target}'
    with tempfile.TemporaryDirectory(prefix='.herdr-work-package-', dir=output) as temp:
        stage = Path(temp) / name
        (stage / 'bin').mkdir(parents=True)
        for label, source in binaries.items():
            dest = stage / 'bin' / f'{label}{suffix}'
            shutil.copy2(source, dest)
        shutil.copy2(ROOT / 'scripts/install.py', stage / 'install.py')
        shutil.copy2(ROOT / 'README.md', stage / 'README.md')
        shutil.copy2(ROOT / 'herdr-plugin.toml', stage / 'herdr-plugin.toml')
        (stage / 'scripts').mkdir()
        shutil.copy2(ROOT / 'scripts/hook.py', stage / 'scripts/hook.py')
        shutil.copy2(ROOT / 'LICENSE', stage / 'LICENSE')
        shutil.copy2(ROOT / 'CHANGELOG.md', stage / 'CHANGELOG.md')
        (stage / 'licenses').mkdir()
        for label, source in {
            'herdr-projects': 'crates/herdr-work-projects/LICENSE',
            'herdr-sidebar': 'crates/herdr-work-code/LICENSE',
        }.items():
            shutil.copy2(ROOT / source, stage / f'licenses/{label}.txt')
        info['files'] = {p.relative_to(stage).as_posix(): digest(p) for p in sorted(stage.rglob('*')) if p.is_file()}
        (stage / 'MANIFEST.json').write_text(json.dumps(info, indent=2) + '\n')
        archive = Path(temp) / (name + ('.zip' if suffix else '.tar.gz'))
        if suffix:
            with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_DEFLATED) as z:
                for path in sorted(stage.rglob('*')):
                    if path.is_file(): z.write(path, f'{name}/{path.relative_to(stage).as_posix()}')
        else:
            with tarfile.open(archive, 'w:gz') as tar:
                tar.add(stage, arcname=name)
        archive.with_name(archive.name + '.sha256').write_text(f'{digest(archive)}  {archive.name}\n')
        verify(archive)
        return publish_archive(archive, output)


def verify(archive):
    archive = Path(archive)
    expected = archive.with_name(archive.name + '.sha256').read_text().strip().split()
    if len(expected) != 2 or expected[1] != archive.name or expected[0] != digest(archive):
        raise ValueError('Archive checksum mismatch')
    with tempfile.TemporaryDirectory(prefix='herdr-work-verify-') as temp:
        dest = Path(temp)
        if archive.name.endswith('.zip'):
            with zipfile.ZipFile(archive) as z:
                for name in z.namelist():
                    p = Path(name)
                    if p.is_absolute() or '..' in p.parts: raise ValueError('Archive path escapes root')
                z.extractall(dest)
        else:
            with tarfile.open(archive, 'r:gz') as tar: tar.extractall(dest, filter='data')
        roots = list(dest.iterdir())
        if len(roots) != 1 or not roots[0].is_dir(): raise ValueError('Expected one archive root')
        root = roots[0]
        manifest = json.loads((root / 'MANIFEST.json').read_text())
        if manifest.get('product') != 'HerdrWork': raise ValueError('Unsupported product identity')
        if manifest['target'] not in TARGETS: raise ValueError('Unexpected platform')
        if manifest['projects_backend'] != TARGETS[manifest['target']]['backend']: raise ValueError('Platform backend capability differs')
        expected_files = set(manifest['files']) | {'MANIFEST.json'}
        actual_files = {p.relative_to(root).as_posix() for p in root.rglob('*') if p.is_file()}
        if expected_files != actual_files: raise ValueError('Archive file set differs from manifest')
        for rel, sha in manifest['files'].items():
            path = root / rel
            if path.is_symlink() or not path.resolve().is_relative_to(root.resolve()) or digest(path) != sha:
                raise ValueError(f'Manifest checksum mismatch: {rel}')
        suffix = '.exe' if 'windows' in manifest['target'] else ''
        if manifest['projects_backend']:
            required.add('bin/herdr-work-projects')
        if not required <= actual_files: raise ValueError('Required executable or license missing')
        return manifest

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='action', required=True)
    p = sub.add_parser('create'); p.add_argument('--target', required=True, choices=TARGETS); p.add_argument('--output', default='dist'); p.add_argument('--tag'); p.add_argument('--keep-archives', type=int, default=1)
    p = sub.add_parser('verify'); p.add_argument('archives', nargs='+')
    args = parser.parse_args()
    if args.action == 'create': print(package(args.target, args.output, args.tag, keep_archives=args.keep_archives))
    else:
        for archive in args.archives: print(json.dumps(verify(archive), indent=2))
