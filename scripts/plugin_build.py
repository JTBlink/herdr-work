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
from package import package, verify
from install import install


def host_target():
    target = {('Darwin', 'arm64'): 'aarch64-apple-darwin',
              ('Linux', 'x86_64'): 'x86_64-unknown-linux-gnu'}.get((platform.system(), platform.machine()))
    if not target:
        raise ValueError('Plugin builds support macOS Apple Silicon and Linux x86_64 only')
    return target


def install_archive(archive):
    verify(archive)
    with tempfile.TemporaryDirectory(prefix='herdr-work-plugin-install-') as temp:
        extract = Path(temp)
        with tarfile.open(archive) as bundle:
            bundle.extractall(extract, filter='data')
        install(next(extract.iterdir()), ROOT)


def fetch():
    target = host_target()
    version = (ROOT / 'VERSION').read_text().strip()
    name = f'herdr-work-{version}-{target}.tar.gz'
    base = f'https://github.com/JTBlink/herdr-work/releases/download/v{version}/'
    with tempfile.TemporaryDirectory(prefix='herdr-work-plugin-') as temp:
        temp = Path(temp)
        for file in (name, name + '.sha256'):
            urllib.request.urlretrieve(base + file, temp / file)
        install_archive(temp / name)


def prepare(from_source=False, *, build_only=False):
    if build_only and not from_source:
        raise ValueError('--build-only requires --from-source')
    if from_source:
        target = host_target()
        print(f'编译 HerdrWork（{target}）…', flush=True)
        subprocess.run([sys.executable, str(ROOT / 'scripts/build.py'), '--target', target], check=True)
        print('打包并校验，当前平台仅保留最新一份归档…', flush=True)
        archive = package(target, ROOT / 'dist', keep_archives=1)
        print(f'归档：{archive}\n校验和：{archive}.sha256', flush=True)
        if build_only:
            return archive
        install_archive(archive)
    else:
        fetch()
    from configure_keys import setup
    setup()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--from-source', action='store_true')
    parser.add_argument('--build-only', action='store_true', help='Requires --from-source; retain archive without installing')
    args = parser.parse_args()
    if args.build_only and not args.from_source:
        parser.error('--build-only requires --from-source')
    prepare(args.from_source, build_only=args.build_only)
