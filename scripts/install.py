"""Install HerdrWork, retaining owned legacy binaries and receipts for safe upgrade/rollback."""
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
LEGACY_RECEIPT = 'workdesk-install.json'
RECEIPTS = (RECEIPT, LEGACY_RECEIPT)
BACKUPS = ('.herdr-work-previous', '.workdesk-previous')
ALLOWED = {'herdr-work', 'herdr-work.exe', 'herdr-work-projects',
           'workdesk', 'workdesk.exe', 'workdesk-projects'}


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
    return next((prefix / name for name in RECEIPTS if (prefix / name).exists()), prefix / RECEIPT)


def validate_receipt(prefix, data):
    for file in data['files']:
        path = Path(file)
        if path.parent != prefix / 'bin' or path.name not in ALLOWED:
            raise ValueError('Invalid install receipt')
    return data


def read_receipt(prefix):
    return validate_receipt(prefix, json.loads(receipt_file(prefix).read_text()))


def write_receipt(prefix, data, name=RECEIPT):
    if name not in RECEIPTS:
        raise ValueError('Invalid receipt name')
    with tempfile.NamedTemporaryFile(mode='w', dir=prefix, delete=False) as stream:
        temp = Path(stream.name)
        try:
            stream.write(json.dumps(data, indent=2) + '\n')
            stream.close()
            os.replace(temp, prefix / name)
        finally:
            temp.unlink(missing_ok=True)


def clean_receipts(prefix, keep):
    for name in RECEIPTS:
        if name != keep and (prefix / name).exists():
            validate_receipt(prefix, json.loads((prefix / name).read_text()))
            (prefix / name).unlink()


def install(root, prefix):
    root, prefix = Path(root).resolve(), Path(prefix).expanduser().resolve()
    if root == prefix or prefix.is_relative_to(root):
        raise ValueError('Install prefix must be outside the extracted archive')
    manifest = json.loads((root / 'MANIFEST.json').read_text())
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
    backup = prefix / BACKUPS[0]
    if previous:
        with tempfile.TemporaryDirectory(dir=prefix) as temp:
            staged = Path(temp)
            for file in previous['files']:
                source = Path(file)
                if not source.is_file():
                    raise ValueError('Previous executable missing; repair installation first')
                shutil.copy2(source, staged / source.name)
            shutil.copy2(receipt, staged / receipt.name)
            (staged / 'receipt-name').write_text(receipt.name)
            if backup.exists():
                shutil.rmtree(backup)
            shutil.copytree(staged, backup)
    installed = []
    try:
        for source in binaries:
            dest = bindir / source.name
            replace(source, dest)
            installed.append(str(dest))
        # Existing Agent shims may still execute the old backend filename. Only
        # an alias already owned by our previous receipt is retained on upgrade.
        legacy = bindir / 'workdesk-projects'
        canonical = bindir / 'herdr-work-projects'
        if str(legacy) in owned and str(canonical) in installed:
            replace(canonical, legacy)
            installed.append(str(legacy))
        for file in owned - set(installed):
            Path(file).unlink(missing_ok=True)
        write_receipt(prefix, {'version': manifest['version'], 'sha': manifest['sha'], 'files': installed})
        clean_receipts(prefix, RECEIPT)
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
    backup = next((prefix / name for name in BACKUPS if (prefix / name).exists()), prefix / BACKUPS[0])
    name = (backup / 'receipt-name').read_text() if (backup / 'receipt-name').exists() else receipt_file(backup).name
    if name not in RECEIPTS:
        raise ValueError('Invalid rollback receipt name')
    old = validate_receipt(prefix, json.loads((backup / name).read_text()))
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
    write_receipt(prefix, old, name)
    clean_receipts(prefix, name)


def uninstall(prefix):
    prefix = Path(prefix).expanduser().resolve()
    data = read_receipt(prefix)
    for file in data['files']:
        Path(file).unlink(missing_ok=True)
    for name in RECEIPTS:
        if (prefix / name).exists():
            validate_receipt(prefix, json.loads((prefix / name).read_text()))
            (prefix / name).unlink()
    for name in BACKUPS:
        backup = prefix / name
        if backup.exists():
            validate_receipt(prefix, json.loads(receipt_file(backup).read_text()))
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
