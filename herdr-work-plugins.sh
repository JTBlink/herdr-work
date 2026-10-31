#!/bin/sh
# Build, package and install HerdrWork through Herdr's official commands.
set -eu

usage() {
    cat <<'EOF'
用法：sh herdr-work-plugins.sh [--from-source | --build-only | --release]

默认自动编译、打包、安装并链接当前源码中的 HerdrWork 插件。
归档与 SHA256 校验和保存在仓库 dist/，当前平台只保留最新一份。
安装时配置默认打开快捷键 Ctrl+B → F；冲突或已有配置时保留用户设置。
  --from-source  编译、打包并安装当前源码（默认）
  --build-only   仅编译打包，不安装，不需要 Herdr
  --release      通过 Herdr 官方安装 VERSION 对应的 GitHub 标签
  -h, --help     显示帮助

依赖：Python 3.12+；源码模式需要 Rust 和平台编译工具，安装需要 Herdr 0.9.3+。
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
    --build-only) mode=--build-only ;;
    --release) mode=--release ;;
    -h|--help) usage; exit 0 ;;
    *) usage >&2; exit 2 ;;
esac

required_tools=python3
if [ "$mode" != --build-only ]; then
    required_tools="$required_tools herdr"
fi
if [ "$mode" != --release ]; then
    required_tools="$required_tools cargo"
fi
for tool in $required_tools; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        printf '缺少依赖：%s，请安装并加入 PATH。\n' "$tool" >&2
        exit 1
    fi
done
python3 -c 'import sys; sys.exit(0 if sys.version_info >= (3, 12) else "需要 Python 3.12+")'

script_dir=${0%/*}
if [ "$script_dir" = "$0" ]; then
    script_dir=.
fi
repo_root=$(CDPATH= cd -- "$script_dir" && pwd)
cd "$repo_root"
python3 scripts/plugin.py "$mode"
if [ "$mode" = --build-only ]; then
    printf 'HerdrWork 编译打包完成，产物位于 %s/dist/。\n' "$repo_root"
elif [ "$mode" = --release ]; then
    printf 'HerdrWork 发行版插件安装完成（jtblink.herdr-work）。\n'
else
    printf 'HerdrWork 插件安装完成（jtblink.herdr-work），本地入口位于 %s/bin/。\n' "$repo_root"
fi
