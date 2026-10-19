"""Install HerdrWork in an owned prefix with atomic replacement and rollback."""
import argparse
import hashlib
import json
import os
import platform
from pathlib import Path
import shutil
import subprocess
import tempfile

RECEIPT = 'herdr-work-install.json'
BACKUP = '.herdr-work-previous'
ALLOWED = {'herdr-work', 'herdr-work.exe', 'herdr-work-projects'}


def digest(file):
    with file.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def replace(source, dest):
    dest.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=dest.parent, delete=False) as stream:
        temp = Path(stream.name)
    try:
        shutil.copy2(source, temp)
        temp.chmod(0o755)
        if platform.system() == 'Darwin':
            subprocess.run(['codesign', '--force', '--sign', '-', str(temp)], check=True, capture_output=True)
        os.replace(temp, dest)
    finally:
        temp.unlink(missing_ok=True)


def receipt_file(prefix):
    return prefix / RECEIPT


def validate_receipt(prefix, data):
    for file in data['files']:
        path = Path(file)
        if path.parent != prefix / 'bin' or path.name not in ALLOWED:
            raise ValueError('Invalid install receipt')
    return data


def read_receipt(prefix):
    return validate_receipt(prefix, json.loads(receipt_file(prefix).read_text()))


def write_receipt(prefix, data):
    with tempfile.NamedTemporaryFile(mode='w', dir=prefix, delete=False) as stream:
        temp = Path(stream.name)
        try:
            stream.write(json.dumps(data, indent=2) + '\n')
            stream.close()
            os.replace(temp, prefix / RECEIPT)
        finally:
            temp.unlink(missing_ok=True)


def install(root, prefix):
    root, prefix = Path(root).resolve(), Path(prefix).expanduser().resolve()
    if root == prefix or prefix.is_relative_to(root):
        raise ValueError('Install prefix must be outside the extracted archive')
    manifest = json.loads((root / 'MANIFEST.json').read_text())
    if manifest.get('product') != 'HerdrWork':
        raise ValueError('Unsupported product identity')
    actual = {p.relative_to(root).as_posix() for p in root.rglob('*') if p.is_file()}
    if actual != set(manifest['files']) | {'MANIFEST.json'}:
        raise ValueError('Archive file set differs from manifest')
    for rel, sha in manifest['files'].items():
        file = root / rel
        if file.is_symlink() or not file.resolve().is_relative_to(root) or not file.is_file():
            raise ValueError(f'Invalid archive path: {rel}')
        if sha != digest(file):
            raise ValueError(f'Checksum mismatch: {rel}')
    binaries = sorted((root / 'bin').iterdir())
    if not binaries or any(not p.is_file() or p.name not in ALLOWED for p in binaries):
        raise ValueError('Unexpected archive executable')
    bindir = prefix / 'bin'
    bindir.mkdir(parents=True, exist_ok=True)
    receipt = receipt_file(prefix)
    previous = read_receipt(prefix) if receipt.exists() else None
    owned = set(previous['files']) if previous else set()
    if any((bindir / p.name).exists() and str(bindir / p.name) not in owned for p in binaries):
        raise ValueError('Unowned executable exists; choose a new prefix')
    backup = prefix / BACKUP
    if previous:
        with tempfile.TemporaryDirectory(dir=prefix) as temp:
            staged = Path(temp)
            for file in previous['files']:
                source = Path(file)
                if not source.is_file():
                    raise ValueError('Previous executable missing; repair installation first')
                shutil.copy2(source, staged / source.name)
            shutil.copy2(receipt, staged / receipt.name)
            if backup.exists():
                shutil.rmtree(backup)
            shutil.copytree(staged, backup)
    installed = []
    try:
        for source in binaries:
            dest = bindir / source.name
            replace(source, dest)
            installed.append(str(dest))
        for file in owned - set(installed):
            Path(file).unlink(missing_ok=True)
        write_receipt(prefix, {'version': manifest['version'], 'sha': manifest['sha'], 'files': installed})
    except Exception:
        if previous:
            rollback(prefix)
            for file in set(installed) - owned:
                Path(file).unlink(missing_ok=True)
        else:
            for file in installed:
                Path(file).unlink(missing_ok=True)
        raise
    return installed


def rollback(prefix):
    prefix = Path(prefix).expanduser().resolve()
    backup = prefix / BACKUP
    old = validate_receipt(prefix, json.loads((backup / RECEIPT).read_text()))
    current = read_receipt(prefix)
    for file in old['files']:
        dest = Path(file)
        if not (backup / dest.name).is_file():
            raise ValueError('Rollback executable missing')
        if dest.exists() and file not in current['files']:
            raise ValueError('Unowned executable exists; cannot roll back')
    for file in old['files']:
        replace(backup / Path(file).name, Path(file))
    for file in set(current['files']) - set(old['files']):
        Path(file).unlink(missing_ok=True)
    write_receipt(prefix, old)


def uninstall(prefix):
    prefix = Path(prefix).expanduser().resolve()
    data = read_receipt(prefix)
    for file in data['files']:
        Path(file).unlink(missing_ok=True)
    (prefix / RECEIPT).unlink()
    backup = prefix / BACKUP
    if backup.exists():
        validate_receipt(prefix, json.loads((backup / RECEIPT).read_text()))
        shutil.rmtree(backup)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prefix', default=str(Path.home() / '.local'))
    parser.add_argument('--archive-root', default=str(Path(__file__).resolve().parent))
    group = parser.add_mutually_exclusive_group()
    group.add_argument('--uninstall', action='store_true')
    group.add_argument('--rollback', action='store_true')
    args = parser.parse_args()
    if args.uninstall:
        uninstall(args.prefix)
    elif args.rollback:
        rollback(args.prefix)
    else:
        print('\n'.join(install(args.archive_root, args.prefix)))
