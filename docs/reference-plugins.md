# 参考插件源码

Sidebar 参考源码保存在根目录 `reference-plugins/`，用于本地阅读、技术验证和重构时的行为对照。
该目录已加入 `.gitignore`，完整副本不提交到本仓库；克隆本仓库不会自动获得它们。

2026-10-18 已在当前工作目录重新下载下列固定 commit 的归档，核对归档根目录、package 版本及 LICENSE，并阅读关键实现；未构建或启动插件。后续实施路径见[源码映射](upstream/source-map.md)。

```text
reference-plugins/
└── herdr-sidebar/
```

## 来源与固定版本

当前副本来自 GitHub 对应 commit 的源码归档，保留完整目录、测试、构建文件和许可证，没有嵌套 `.git` 历史。
这些版本与现有技术方案一致，不表示上游最新版本。

| 插件 | 版本 | 固定 commit | Cargo 清单位置（相对参考目录） |
| --- | --- | --- | --- |
| [Sidebar](https://github.com/alexarthurs/herdr-sidebar/tree/081ed133f3faa377438b51ce435fe8a87c439593) | 0.15.1 | `081ed133f3faa377438b51ce435fe8a87c439593` | `herdr-sidebar/plugins/herdr-sidebar/Cargo.toml` |

## 重新获取

从仓库根目录下载固定 commit 的归档并展开。以下命令仅在对应目标目录不存在时执行，避免覆盖本地改动。

```bash
mkdir -p reference-plugins/herdr-sidebar
curl --fail --location https://codeload.github.com/alexarthurs/herdr-sidebar/tar.gz/081ed133f3faa377438b51ce435fe8a87c439593 -o reference-plugins/sidebar.tar.gz
tar -xzf reference-plugins/sidebar.tar.gz --strip-components=1 -C reference-plugins/herdr-sidebar

```

## 后续重构约定

- 生产源码位于 `crates/` 的五个包，统一 workspace 与锁文件，见[内部模块架构](architecture/herdr-work-internal-modules.md)。
- 参考副本保留原貌。需长期保留的提取、适配和重构代码进入本仓库正常跟踪的目录，不能只留在被忽略的副本中。
- 提取代码时记录来源 commit，保留相关版权与 MIT 许可文本，并移植对应行为测试。
- 参考源码中的插件清单与启动器用于分析；下载不会安装、注册或启用插件，也不代表工作台已完成合并。
- 归档来自固定 commit 的 GitHub codeload 地址，下载时核对根目录与清单版本；没有嵌套 Git 历史，不把本地目录当作可独立证明 commit 的 checkout。参考副本不作为构建或测试入口。
