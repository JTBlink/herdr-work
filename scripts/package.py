"""Create and verify platform archives, original licenses, provenance and SHA256."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import tarfile
import tempfile
import zipfile
from build import ROOT, TARGETS, metadata


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def package(target, output, tag=None):
    info = metadata(target, tag)
    output = Path(output)
    output.mkdir(parents=True, exist_ok=True)
    suffix = '.exe' if 'windows' in target else ''
    binaries = {'herdr-work': ROOT / f'target/{target}/release/herdr-work{suffix}'}
    if info['projects_backend']:
        binaries['herdr-work-projects'] = ROOT / f'target/{target}/release/herdr-work-projects'
    for path in binaries.values():
        if not path.is_file() or path.stat().st_size == 0:
            raise ValueError(f'Compiled binary missing: {path}')
    name = f'herdr-work-{info["version"]}-{target}'
    with tempfile.TemporaryDirectory(prefix='herdr-work-package-') as temp:
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
        for label in ['herdr-projects', 'herdr-sidebar', 'herdr-reviewr']:
            shutil.copy2((ROOT / 'crates/herdr-work-code/LICENSE' if label == 'herdr-sidebar' else ROOT / f'upstream/{label}/LICENSE'), stage / f'licenses/{label}.txt')
        info['files'] = {p.relative_to(stage).as_posix(): digest(p) for p in sorted(stage.rglob('*')) if p.is_file()}
        (stage / 'MANIFEST.json').write_text(json.dumps(info, indent=2) + '\n')
        archive = output / (name + ('.zip' if suffix else '.tar.gz'))
        if suffix:
            with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_DEFLATED) as z:
                for path in sorted(stage.rglob('*')):
                    if path.is_file(): z.write(path, f'{name}/{path.relative_to(stage).as_posix()}')
        else:
            with tarfile.open(archive, 'w:gz') as tar:
                tar.add(stage, arcname=name)
    archive.with_name(archive.name + '.sha256').write_text(f'{digest(archive)}  {archive.name}\n')
    verify(archive)
    return archive


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
        required = {'bin/herdr-work' + suffix, 'install.py', 'LICENSE'} | {f'licenses/{name}.txt' for name in ['herdr-projects','herdr-sidebar','herdr-reviewr']}
        if manifest['projects_backend']:
            required.add('bin/herdr-work-projects')
        if not required <= actual_files: raise ValueError('Required executable or license missing')
        return manifest

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='action', required=True)
    p = sub.add_parser('create'); p.add_argument('--target', required=True, choices=TARGETS); p.add_argument('--output', default='dist'); p.add_argument('--tag')
    p = sub.add_parser('verify'); p.add_argument('archives', nargs='+')
    args = parser.parse_args()
    if args.action == 'create': print(package(args.target, args.output, args.tag))
    else:
        for archive in args.archives: print(json.dumps(verify(archive), indent=2))
