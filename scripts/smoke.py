"""Install a native archive and exercise the actual CLI and workbench."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from package import verify
from install import install, rollback
from ui_smoke import ui_smoke


def output(command, **kwargs):
    return subprocess.check_output([str(item) for item in command], text=True, timeout=60, **kwargs)


def smoke(archive):
    manifest = verify(archive)
    with tempfile.TemporaryDirectory(prefix='herdr-work-install-test-') as temp:
        root = Path(temp)
        extract = root / 'archive'
        extract.mkdir()
        if str(archive).endswith('.zip'):
            with zipfile.ZipFile(archive) as bundle:
                bundle.extractall(extract)
        else:
            with tarfile.open(archive) as bundle:
                bundle.extractall(extract, filter='data')
        stage = next(extract.iterdir())
        prefix = root / 'installed'
        install(stage, prefix)
        binary = prefix / 'bin' / ('herdr-work.exe' if 'windows' in manifest['target'] else 'herdr-work')
        if output([binary, '--version']).strip() != f"herdr-work {manifest['version']}":
            raise ValueError('Installed binary version differs')
        profile = root / 'profile'
        result = json.loads(output([binary, '--home', profile, 'doctor']))
        if result['product'] != 'HerdrWork':
            raise ValueError('Installed application identity differs')
        ui_smoke(binary)
        if os.name != 'nt':
            subprocess.run([sys.executable, str(Path(__file__).parent / 'tests/viewer_close_fixture.py'),
                            str(binary)], check=True, timeout=60)
            subprocess.run([sys.executable, str(Path(__file__).parent / 'tests/vscode_settings_fixture.py'),
                            str(binary)], check=True, timeout=60)
        install(stage, prefix)
        rollback(prefix)
        output([binary, '--version'])
        subprocess.run([sys.executable, str(stage / 'install.py'), '--prefix', str(prefix), '--uninstall'], check=True)
        if binary.exists() or not profile.exists():
            raise ValueError('Uninstall left the binary or removed user data')
    print(f'Installed, executed and uninstalled {Path(archive).name}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archives', required=True)
    args = parser.parse_args()
    archives = [*Path(args.archives).glob('*.tar.gz'), *Path(args.archives).glob('*.zip')]
    if len(archives) != 1:
        raise ValueError('Smoke requires exactly one native platform archive')
    smoke(archives[0])
