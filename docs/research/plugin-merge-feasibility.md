---
title: Herdr 工作台插件合并可行性调研
status: researched
date: 2026-10-18
tags:
  - research
  - herdr
  - coding-workbench
---

# Herdr 工作台插件合并可行性调研

## 结论

可以整合为一个工作台，但“统一入口”“只注册一个插件”“合成一个程序”是三种不同方案。
基于当前安装的固定版本，**推荐先统一入口，保留三个插件及各自程序；真正单插件发行需要维护适配后的源码**。
不能把三个现有目录放到一个插件清单后，就认为原功能、升级和状态管理仍会自动正常工作。

| 目标 | 源码核实后的判断 | 主要工作 |
|---|---|---|
| 一个入口，三插件继续安装 | 可作为最小方案；实际仍有三个插件身份 | 新增工作台命令，调用已有 actions；统一布局与快捷键 |
| 一个插件身份，多个内部程序 | 技术上可设计；现有程序必须适配 | 统一清单、修正插件 ID、隔离状态、统一事件与更新 |
| 一个可执行程序，多个界面模块 | 可进一步重构，当前没有可直接组合的统一界面接口证据 | 整合终端事件循环、界面状态、窗格识别及进程管理 |

“可设计”属于依据源码作出的工程推断，**本次没有实现或运行合包 PoC**。

## 范围与证据

只读检查本地注册表和安装 checkout。三个 checkout 的 Git HEAD 均与注册表记录一致，
`git status --short` 均无输出。以下链接固定到该 commit，不表示上游最新版本。

| 插件 | 版本 | 固定 commit | 本次读取的清单位置 |
|---|---|---|---|
| Projects | 1.2.2 | `92a2813be9a7c2f8cda737f6b78ed21e8413c3ce` | 仓库根 `herdr-plugin.toml` |
| Sidebar | 0.15.1 | `081ed133f3faa377438b51ce435fe8a87c439593` | `plugins/herdr-sidebar/herdr-plugin.toml` |

注册表：`~/.config/herdr/plugins.json`，其中 `plugin_root` 是清单目录，
`source.managed_path` 是源码 checkout 根目录；Sidebar 两者不同。

## 宿主架构核实

本机 Herdr 为 0.9.4；本节按上游 `v0.9.4` 源码核对。清单有一个 `id`，
但 build/startup/actions/events/panes 均可有多个条目，因此一个插件运行多个程序属于宿主现有能力。
清单结构没有依赖、include 或子插件声明；把多个 manifest 放进目录不会形成原生依赖管理。
action ID 和 pane ID 在同一清单内分别不得重复。
依据：[manifest 结构](https://github.com/herdrdev/herdr/blob/v0.9.4/src/app/api/plugins/manifest.rs#L11-L34)、
[ID 重复校验](https://github.com/herdrdev/herdr/blob/v0.9.4/src/app/api/plugins/manifest.rs#L320-L356)。

当前扩展界面由终端窗格承载，没有原生非终端插件 UI 接口；合为一个插件包不会自动形成一个统一编辑器界面。
依据：[该 tag 的插件文档](https://github.com/herdrdev/herdr/blob/v0.9.4/docs/versions/0.9.3/website/src/content/docs/plugins.mdx#L31-L32)。

普通插件命令以 plugin root 为 CWD；宿主按注册 ID 注入 root/config/state 环境。
pane 创建会保护这些环境变量，并记录 plugin ID / entrypoint；包装脚本可以为子进程设置模块目录，
但不会因此产生独立的宿主插件身份。依据：[runtime](https://github.com/herdrdev/herdr/blob/v0.9.4/src/app/api/plugins/runtime.rs#L125)、[env](https://github.com/herdrdev/herdr/blob/v0.9.4/src/app/api/plugins/env.rs#L17-L32)、[pane 环境与身份](https://github.com/herdrdev/herdr/blob/v0.9.4/src/app/api/plugins/panes.rs#L254-L318)。

安装执行 build，link 跳过 build；startup 在 server restore 后运行，不因 link/enable 自动触发，
也不是宿主提供的守护进程监督。统一包需自行处理首次启用和后台生命周期。
依据：[该 tag 的插件文档](https://github.com/herdrdev/herdr/blob/v0.9.4/docs/versions/0.9.3/website/src/content/docs/plugins.mdx#L218-L249)。

## 许可证与构建条件

三个固定版本的仓库根目录都有实际 `LICENSE`，内容均为 MIT。
版权署名分别为 Elias Stravik、Alex Arthurs、Dmitry Persiyanov。
合并发行应保留各自版权和许可文本；本次没有对全部传递依赖逐项作许可证审计。
依据：[Projects LICENSE][p-license] / [Cargo][p-cargo]、
[Sidebar LICENSE][s-license] / [Cargo][s-cargo]、

三者均为 Rust 项目，主界面依赖 Ratatui 0.30，但这不代表共享同一个 UI 接口。
源码整合必须重新验证统一工具链和 Cargo.lock；不能据相同界面库推断可直接静态拼接。

## 插件身份与窗格入口

| 项目 | 已核实行为 | 单插件适配点 |
|---|---|---|
| Projects | `open_board` 和 popup 创建显式传 `--plugin herdr-projects --entrypoint board`；doctor 按固定 ID 查注册表 | 统一读取运行身份，并修正诊断、入口和生成的快捷键 |
| Sidebar | `plugin.pane.open` 参数显式传 `plugin_id: herdr-sidebar`，入口为 `sidebar` / `source-control` | 改成工作台身份；保留或映射两个入口 |

依据：[Projects pane API][p-api]、[Projects doctor][p-doctor]、
[Sidebar IPC][s-ipc]、[Sidebar view identity][s-state]、

例如 `projects-open`、`review-open`。模块内部可继续使用原子命令，但宿主 action 与快捷键映射需要同步。

## 路径、CWD 与数据

**插件文件目录与被操作的项目目录必须分开处理。**

- Projects `bin/run.sh` 根据脚本自身位置找根目录，在根目录读取 Cargo 版本、源码和预编译产物；
  原启动脚本需要完整相对目录或重新编写。其任务数据默认在 `~/.herdr-projects/data`，
  配置在 `~/.herdr-projects/settings`，并支持 `HERDR_PROJECTS_HOME` 及分目录变量。
  依据：[Projects launcher][p-run]、[Projects files][p-files]。
- Sidebar 安装脚本默认在自身父目录找 Cargo 和 `target/release`。
  Unix 创建窗格时，通过专用环境变量传入项目 CWD，程序启动后再切换目录，
  避免相对可执行路径按项目目录解析。不能在统一包装脚本里简单 `cd` 到项目目录后照搬原命令。
  依据：[Sidebar fetch/build][s-fetch]、[IPC][s-ipc]、[main][s-main]。
- Sidebar 优先读取 `HERDR_PLUGIN_STATE_DIR`，无值时回退原 `herdr-sidebar` 状态目录；
  其中保存 `state.json`、编辑器命令、文件树和 SCM 状态。
  workspace action lock 使用 `HERDR_PLUGIN_STATE_DIR`。
  合包后应按模块分配目录，不能把不同模块的数据混在同一个状态命名空间。

## 窗格归属与重复实例

Sidebar 使用 `herdr-sidebar-explorer`、`herdr-sidebar-git` 等 metadata token，
结合 label、心跳与启动标记识别自己的窗格；启动器可清理失效窗格并按布局补建。
如果原插件与合包版本同时启用而沿用这些 token，会互相认领。
这些 token 是应用层身份标记，不能简单等同于宿主注册的 plugin ID。
依据：[Sidebar launch][s-launch]、[Sidebar state][s-state]、[Sidebar ensure][s-ensure]。

并非只按创建它的插件身份筛选。原版与合包版使用同名程序时可能彼此认领或关闭；
若把程序改名为 `herdr-workbench`，现有识别规则又会失效。

Projects 任务 hook 按 socket 与 task link 的 pane ID 找目标，再校验工作目录，
不会把任意 review pane 当作任务执行者。整合层应保留 task、workspace、pane、worktree 的真实关联，
而不是只靠界面标题猜测。
依据：[Projects hook][p-hook]。

因此，迁移应先停用原三插件的自动钩子，并在隔离配置中测试合包；
历史窗格的 token、进程名和数据迁移需要明确兼容策略。
这是后续实施建议，本次没有停用、关闭或迁移现有插件。

## 事件、快捷键与生命周期

| 插件 | 清单已声明的启动 / 事件 | 合包关注点 |
|---|---|---|
| Projects | startup；agent detected/status changed、pane closed、worktree removed | 保留任务推进与恢复路径；hook 启动 detached keeper |
| Sidebar | pane focused、tab created/focused、workspace created/focused | 事件可能很频繁；自动补窗格、跟随目录与布局要幂等 |

依据：[Projects 清单][p-manifest] / [hook][p-hook]、
[Sidebar 清单][s-manifest] / [ensure][s-ensure]、

Projects keeper 在 board 关闭时仍执行周期工作，每个 data folder 一份，
自身还有心跳、子进程和更新交接。合包后仅关闭看板不等于结束后台功能，
需设计工作台禁用、更新和卸载时的后台进程策略。
依据：[Projects keeper][p-keeper]。

Projects 可以生成宿主 config 中 `herdr-projects.open` 的快捷键配置；
合包应统一宿主快捷键命名，保留独立 pane 时无需把三个 TUI 的全部按键强行合并。
若做成一个 TUI，则必须重新协调焦点、raw mode、alternate screen、鼠标和事件循环。

## 安装、更新与平台边界

Projects launcher 按 Cargo 版本下载上游 release，可回退源码构建；程序还有原仓库的自更新器。
Sidebar fetch/build 默认下载原仓库匹配版本，程序更新器还显式查找 `herdr-sidebar` 注册项和原 GitHub source。
如果自定义工作台 fork 仍保留这些更新路径，后续可能把适配后的程序换回未适配原版。
**单插件发行必须由工作台统一固定三个模块版本、校验和、更新与回滚。**
依据：[Projects run][p-run] / [install updater][p-install]、

完整工作台当前可复用的平台交集是 **Linux 和 macOS**。
Projects 清单仅声明这两个平台，源码直接使用 `std::os::unix` 和 Unix libc；
Sidebar Windows 还采用 GUI subsystem 的 ensure sidecar，与 Unix 直接 pane 启动路径不同。
依据：[Projects 清单][p-manifest] / [files][p-files]、[Sidebar 清单][s-manifest] / [Cargo][s-cargo]、

## 最少改造路线与验收

1. **先统一入口**：保留三个插件 ID 和数据目录，新工作台只编排 actions 与布局。
   先确认任务选中时如何传递 repo、worktree、workspace、pane；这是低耦合方案，不能称已合成单插件。
2. **需要单插件发行时**：创建一个 manifest 和三个内部二进制；保留独立终端进程，
   对 Projects / Sidebar 的宿主 ID 做小范围 fork，统一 action 命名并给每个模块分配环境与状态目录。
3. **统一生命周期与升级**：关闭模块自行更新入口；打包固定版本，提供已有数据迁移和回滚办法。
   处理 Projects keeper 以及 Sidebar 自动补建，避免旧插件与新插件互相争夺 pane。
4. **验证后才承诺可用**：隔离配置中验证首次安装、重启恢复、worktree 事件并发、
   工作区切换、toggle 去重、审查反馈目标、任务完成流程、禁用/卸载、升级回滚和缺依赖报错。
   Linux/macOS 分别验证；Windows 应作为额外适配项目。

本次只完成固定版本源码和清单核实，没有修改插件、执行合包构建、控制 Herdr 会话、迁移数据或运行上述验收。

## 固定版本源码索引

[p-manifest]: https://github.com/eliasstravik/herdr-projects/blob/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce/herdr-plugin.toml
[p-license]: https://github.com/eliasstravik/herdr-projects/blob/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce/LICENSE
[p-cargo]: https://github.com/eliasstravik/herdr-projects/blob/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce/Cargo.toml
[p-api]: https://github.com/eliasstravik/herdr-projects/blob/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce/crates/core/src/herdr/api.rs#L193-L205
[p-doctor]: https://github.com/eliasstravik/herdr-projects/blob/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce/crates/flow/src/doctor/plugin.rs#L22-L45
[p-run]: https://github.com/eliasstravik/herdr-projects/blob/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce/bin/run.sh
[p-files]: https://github.com/eliasstravik/herdr-projects/blob/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce/crates/core/src/store/files.rs#L26-L53
[p-hook]: https://github.com/eliasstravik/herdr-projects/blob/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce/crates/flow/src/hook.rs#L25-L55
[p-keeper]: https://github.com/eliasstravik/herdr-projects/blob/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce/crates/flow/src/keeper.rs
[p-extras]: https://github.com/eliasstravik/herdr-projects/blob/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce/crates/flow/src/extras/config.rs
[p-install]: https://github.com/eliasstravik/herdr-projects/blob/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce/crates/flow/src/install.rs
[s-manifest]: https://github.com/alexarthurs/herdr-sidebar/blob/081ed133f3faa377438b51ce435fe8a87c439593/plugins/herdr-sidebar/herdr-plugin.toml
[s-license]: https://github.com/alexarthurs/herdr-sidebar/blob/081ed133f3faa377438b51ce435fe8a87c439593/LICENSE
[s-cargo]: https://github.com/alexarthurs/herdr-sidebar/blob/081ed133f3faa377438b51ce435fe8a87c439593/plugins/herdr-sidebar/Cargo.toml
[s-ipc]: https://github.com/alexarthurs/herdr-sidebar/blob/081ed133f3faa377438b51ce435fe8a87c439593/plugins/herdr-sidebar/src/ipc.rs#L137-L189
[s-state]: https://github.com/alexarthurs/herdr-sidebar/blob/081ed133f3faa377438b51ce435fe8a87c439593/plugins/herdr-sidebar/src/state.rs
[s-main]: https://github.com/alexarthurs/herdr-sidebar/blob/081ed133f3faa377438b51ce435fe8a87c439593/plugins/herdr-sidebar/src/main.rs#L207-L235
[s-fetch]: https://github.com/alexarthurs/herdr-sidebar/blob/081ed133f3faa377438b51ce435fe8a87c439593/plugins/herdr-sidebar/scripts/fetch-or-build.sh
[s-launch]: https://github.com/alexarthurs/herdr-sidebar/blob/081ed133f3faa377438b51ce435fe8a87c439593/plugins/herdr-sidebar/src/launch.rs
[s-ensure]: https://github.com/alexarthurs/herdr-sidebar/blob/081ed133f3faa377438b51ce435fe8a87c439593/plugins/herdr-sidebar/src/ensure.rs
[s-updates]: https://github.com/alexarthurs/herdr-sidebar/blob/081ed133f3faa377438b51ce435fe8a87c439593/plugins/herdr-sidebar/src/updates.rs
