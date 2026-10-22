"""Build both HerdrWork product binaries from the root workspace with locked inputs."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tomllib
from cleanup import clean_caches

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {
    'aarch64-apple-darwin': {'runner': 'macos-14', 'backend': True},
    'x86_64-unknown-linux-gnu': {'runner': 'ubuntu-24.04', 'backend': True},
    'x86_64-pc-windows-msvc': {'runner': 'windows-2022', 'backend': False},
}

def metadata(target, tag=None):
    if target not in TARGETS:
        raise ValueError(f'Unsupported target: {target}')
    version = (ROOT / 'VERSION').read_text().strip()
    workspace = tomllib.loads((ROOT / 'Cargo.toml').read_text())
    if workspace['workspace']['package']['version'] != version:
        raise ValueError('VERSION and workspace.package.version differ')
    import re
    if not re.fullmatch(r'0|[1-9]\d*',version.split('.')[0]) or not re.fullmatch(r'(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?',version):
        raise ValueError('VERSION must use semantic versioning')
    plugin=tomllib.loads((ROOT / 'herdr-plugin.toml').read_text())
    if plugin['version'] != version or plugin['id'] != 'jtblink.herdr-work':raise ValueError('Plugin identity/version differ')
    if tag and tag != f'v{version}':
        raise ValueError('Tag and VERSION differ')
    sha = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    return {'product': 'HerdrWork', 'version': version, 'target': target, 'sha': sha,
            'projects_backend': TARGETS[target]['backend'],
            'windows_limitation': None if TARGETS[target]['backend'] else 'Native files/Git/review UI; Projects execution backend is Unix-only'}

def target_dir(root=None):
    root = ROOT if root is None else root
    path = Path(os.environ.get('CARGO_TARGET_DIR', root / 'target')).expanduser()
    return path if path.is_absolute() else root / path


def build(target, tag=None):
    info = metadata(target, tag)
    cleanup = clean_caches(ROOT, apply=True)
    if cleanup['items'] or cleanup['busy']:
        print('Build cache cleanup: ' + json.dumps(cleanup), file=sys.stderr)
    command = ['cargo', 'build', '--release', '--locked', '--target', target,
               '--target-dir', str(target_dir()), '-p', 'herdr-work', '--bin', 'herdr-work']
    if info['projects_backend']:
        command += ['-p', 'herdr-work-projects', '--bin', 'herdr-work-projects']
    subprocess.run(command, cwd=ROOT, check=True)
    return info

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', required=True, choices=TARGETS)
    parser.add_argument('--tag')
    parser.add_argument('--metadata-only', action='store_true')
    args = parser.parse_args()
    print(json.dumps(metadata(args.target, args.tag) if args.metadata_only else build(args.target, args.tag), indent=2))
