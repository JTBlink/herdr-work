#!/bin/sh
# Wrap Herdr's official GitHub install and local link commands.
set -eu

usage() {
    cat <<'EOF'
用法：sh install-plugin.sh [--from-source | --release]

默认从当前源码构建并链接 HerdrWork 插件。
安装时配置默认打开快捷键 Ctrl+B → F；冲突或已有配置时保留用户设置。
  --from-source  从源码构建（需要 Rust 和平台编译工具）
  --release      通过 Herdr 官方安装 VERSION 对应的 GitHub 标签
  -h, --help     显示帮助

依赖：Python 3.12+、Herdr 0.9.3+。
支持 macOS Apple Silicon 和 Linux x86_64。
EOF
}

mode=--from-source
if [ "$#" -gt 1 ]; then
    usage >&2
    exit 2
fi
case "${1:-}" in
    ''|--from-source) ;;
    --release) mode=--release ;;
    -h|--help) usage; exit 0 ;;
    *) usage >&2; exit 2 ;;
esac

for tool in python3 herdr; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        printf '缺少依赖：%s，请安装并加入 PATH。\n' "$tool" >&2
        exit 1
    fi
done
python3 -c 'import sys; sys.exit(0 if sys.version_info >= (3, 12) else "需要 Python 3.12+")'
if [ "$mode" = --from-source ] && ! command -v cargo >/dev/null 2>&1; then
    printf '从源码安装需要 Rust，请安装仓库 rust-toolchain.toml 指定的工具链。\n' >&2
    exit 1
fi

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$repo_root"
if [ "$mode" = --release ]; then
    python3 scripts/plugin.py --release
else
    python3 scripts/plugin.py --from-source
fi
printf 'HerdrWork 插件安装完成（jtblink.herdr-work）。\n'
