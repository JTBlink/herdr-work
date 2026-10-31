"""Install HerdrWork through Herdr's official install/link commands."""
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def install_plugin(from_source=False, *, build_only=False):
    if from_source or build_only:
        command = [sys.executable, str(ROOT / 'scripts/plugin_build.py'), '--from-source']
        if build_only:
            command.append('--build-only')
        subprocess.run(command, cwd=ROOT, check=True)
        if not build_only:
            subprocess.run(['herdr', 'plugin', 'link', str(ROOT)], check=True)
    else:
        version = (ROOT / 'VERSION').read_text().strip()
        subprocess.run(['herdr', 'plugin', 'install', 'JTBlink/herdr-work',
                        '--ref', f'v{version}'], check=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--from-source', action='store_true', help='编译、打包并链接当前源码目录')
    mode.add_argument('--build-only', action='store_true', help='仅编译打包到 dist，不安装或链接')
    mode.add_argument('--release', action='store_true', help='通过 Herdr 安装 VERSION 对应的 GitHub 标签（默认）')
    args = parser.parse_args()
    install_plugin(args.from_source, build_only=args.build_only)
