"""Herdr manifest build hook: prepare verified binaries and the default shortcut."""
import argparse
from pathlib import Path
import platform
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
from build import ROOT
from package import verify
from install import install


def fetch():
    target = {('Darwin', 'arm64'): 'aarch64-apple-darwin',
              ('Linux', 'x86_64'): 'x86_64-unknown-linux-gnu'}.get((platform.system(), platform.machine()))
    if not target:
        raise ValueError('No release archive for this host; build explicitly from source')
    version = (ROOT / 'VERSION').read_text().strip()
    name = f'herdr-work-{version}-{target}.tar.gz'
    base = f'https://github.com/JTBlink/herdr-work/releases/download/v{version}/'
    with tempfile.TemporaryDirectory(prefix='herdr-work-plugin-') as temp:
        temp = Path(temp)
        for file in (name, name + '.sha256'):
            urllib.request.urlretrieve(base + file, temp / file)
        verify(temp / name)
        extract = temp / 'extracted'
        extract.mkdir()
        with tarfile.open(temp / name) as archive:
            archive.extractall(extract, filter='data')
        install(next(extract.iterdir()), ROOT)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--from-source', action='store_true')
    args = parser.parse_args()
    if args.from_source:
        target = {('Darwin', 'arm64'): 'aarch64-apple-darwin',
                  ('Linux', 'x86_64'): 'x86_64-unknown-linux-gnu'}.get((platform.system(), platform.machine()))
        if not target:
            raise ValueError('Unsupported plugin host')
        subprocess.run([sys.executable, str(ROOT / 'scripts/build.py'), '--target', target], check=True)
        from package import package
        with tempfile.TemporaryDirectory(prefix='herdr-work-source-') as temp:
            archive = package(target, Path(temp))
            extract = Path(temp) / 'extract'
            extract.mkdir()
            with tarfile.open(archive) as bundle:
                bundle.extractall(extract, filter='data')
            install(next(extract.iterdir()), ROOT)
    else:
        fetch()
    from configure_keys import setup
    setup()
