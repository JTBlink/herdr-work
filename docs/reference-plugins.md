# 参考插件源码

三个插件的完整源码保存在根目录 `reference-plugins/`，用于本地阅读、技术验证和重构时的行为对照。
该目录已加入 `.gitignore`，完整副本不提交到本仓库；克隆本仓库不会自动获得它们。

2026-10-18 已在当前工作目录重新下载下列固定 commit 的归档，核对归档根目录、package 版本及 LICENSE，并阅读关键实现；未构建或启动插件。后续实施路径见[源码映射](upstream/source-map.md)。

```text
reference-plugins/
├── herdr-projects/
├── herdr-sidebar/
└── herdr-reviewr/
```

## 来源与固定版本

当前副本来自 GitHub 对应 commit 的源码归档，保留完整目录、测试、构建文件和许可证，没有嵌套 `.git` 历史。
这些版本与现有技术方案一致，不表示上游最新版本。

| 插件 | 版本 | 固定 commit | Cargo 清单位置（相对参考目录） |
| --- | --- | --- | --- |
| [Projects](https://github.com/eliasstravik/herdr-projects/tree/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce) | 1.2.2 | `92a2813be9a7c2f8cda737f6b78ed21e8413c3ce` | `herdr-projects/Cargo.toml` |
| [Sidebar](https://github.com/alexarthurs/herdr-sidebar/tree/081ed133f3faa377438b51ce435fe8a87c439593) | 0.15.1 | `081ed133f3faa377438b51ce435fe8a87c439593` | `herdr-sidebar/plugins/herdr-sidebar/Cargo.toml` |
| [Reviewr](https://github.com/persiyanov/herdr-reviewr/tree/859dd5b11a5280fe282de56b52378b20a54fa10a) | 0.46.0 | `859dd5b11a5280fe282de56b52378b20a54fa10a` | `herdr-reviewr/Cargo.toml` |

## 重新获取

从仓库根目录下载固定 commit 的归档并展开。以下命令仅在对应目标目录不存在时执行，避免覆盖本地改动。

```bash
mkdir -p reference-plugins/herdr-projects
curl --fail --location https://codeload.github.com/eliasstravik/herdr-projects/tar.gz/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce -o reference-plugins/projects.tar.gz
tar -xzf reference-plugins/projects.tar.gz --strip-components=1 -C reference-plugins/herdr-projects

mkdir -p reference-plugins/herdr-sidebar
curl --fail --location https://codeload.github.com/alexarthurs/herdr-sidebar/tar.gz/081ed133f3faa377438b51ce435fe8a87c439593 -o reference-plugins/sidebar.tar.gz
tar -xzf reference-plugins/sidebar.tar.gz --strip-components=1 -C reference-plugins/herdr-sidebar

mkdir -p reference-plugins/herdr-reviewr
curl --fail --location https://codeload.github.com/persiyanov/herdr-reviewr/tar.gz/859dd5b11a5280fe282de56b52378b20a54fa10a -o reference-plugins/reviewr.tar.gz
tar -xzf reference-plugins/reviewr.tar.gz --strip-components=1 -C reference-plugins/herdr-reviewr
```

## 后续重构约定

- 工作台根 workspace 的四个成员为 `crates/workdesk-core/`、`crates/workdesk-engine/`、`crates/workdesk-tui/`、`crates/workdesk/`；来源源码放入 `upstream/` 并保留原仓库结构、workspace 和锁文件，通过统一构建入口接入；详见[复用优先架构](architecture/workdesk-architecture.md)。这些源码目录尚未创建。
- 参考副本保留原貌。需长期保留的提取、适配和重构代码进入本仓库正常跟踪的目录，不能只留在被忽略的副本中。
- 提取代码时记录来源 commit，保留相关版权与 MIT 许可文本，并移植对应行为测试。
- 参考源码中的插件清单与启动器用于分析；下载不会安装、注册或启用插件，也不代表工作台已完成合并。
- 归档来自固定 commit 的 GitHub codeload 地址，下载时核对根目录与清单版本；没有嵌套 Git 历史，不把本地目录当作可独立证明 commit 的 checkout。未运行三个上游的构建或测试。
