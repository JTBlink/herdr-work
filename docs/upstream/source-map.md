# 固定源码与导入映射

## 当前获取状态

2026-10-18 三个固定 commit 的源码归档已下载到 `reference-plugins/`；根目录、package 版本及 LICENSE 已核对，关键逻辑已重新阅读。源码获取没有安装、注册或启用插件；随后已完整导入 upstream，实际编译与测试记录以 GitHub Issue 和 CI 为准。

| 来源 | 固定 commit | 已下载目录 | 核对版本 |
| --- | --- | --- | --- |
| Projects | `92a2813be9a7c2f8cda737f6b78ed21e8413c3ce` | `reference-plugins/herdr-projects/` | 1.2.2 |
| Sidebar | `081ed133f3faa377438b51ce435fe8a87c439593` | `reference-plugins/herdr-sidebar/` | 0.15.1 |

版本与下载方式见[参考源码](../reference-plugins.md)。归档不包含 `.git`，固定 commit 来自下载 URL 与归档根目录，不冒充已检查 Git HEAD 的 checkout。

## 已实施目录

用户已确认以下目录规范。实施时将完整来源树复制到受版本管理的 upstream；已完整导入，参考副本保持原貌。四个工作台 crate 平铺在 crates 下，不把来源模块拆进其中。

| 来源根目录 | 目标目录 | 保留内容 |
| --- | --- | --- |
| Projects 完整仓库 | `upstream/herdr-projects/` | 原根 package/CLI、crates/core、crates/flow、crates/board、crates/web、资源和测试 |
| Sidebar 完整仓库 | `upstream/herdr-sidebar/` | 原根结构、plugins/herdr-sidebar package、资源和测试 |

来源根目录下面的相对路径不变。工作台通过 path dependency 和保留的 Projects 后端复用实际实现，不修改来源的模块命名来迎合自有 crate 命名。

工作台根 workspace 显式列出 `herdr-work-core`、`herdr-work-engine`、`herdr-work-tui`、`herdr-work` 四个成员，排除上游项目自动加入；保留来源 workspace 与锁文件。统一构建入口分别编排工作台和后端，记录使用的锁文件、固定 commit、适配补丁及行为测试。

根 workspace 和 Projects 后端分别以原锁文件编译。目录统一不等于扁平化上游结构，也不意味着共用一个 Cargo.lock；暂不降低来源声明的工具链要求。

## 复用与升级约束

- 原业务代码必须实际编译和调用；保留相应行为测试与 fixture。包装 CLI 不代替源码复用，CLI 后端仍由导入源码构建。
- 目录整理补丁与业务适配补丁分开说明；任务阶段、Git 解析和 Diff 算法不因整理目录而重写。
- 导入时保留各自 LICENSE、版权及资源许可，记录引用范围；本次只确认原仓库根 LICENSE，不代替传递依赖许可检查。
- 每次实现列出来源模块、保留行为、适配点、行为差异和验证结果。上游升级以明确 commit 更新，不自动跟随分支。
- 关闭旧更新器、安装下载器和自动开窗格逻辑，通过工作台统一构建和版本验收。

## HerdrWork 必要适配

| 来源文件 | 补丁与实际复用 |
| --- | --- |
| Projects `src/cli/mod.rs` | 增加启动迁移前的只读 `herdr-work-snapshot`，调用原 tasks::list；增加调度暂停/恢复控制。managed 模式跳过旧安装接管及旧数据自动导入，禁止原更新入口。其他 add/delegate/stop/reopen 仍执行原命令逻辑。 |
| Projects `crates/core/src/store/files.rs` | managed 模式的 board_env 为 Agent workspace、重启及实际 launcher 传递托管标记、home、data/config/reports；原模式保持原两项环境。Agent 回调不注入插件 UI 身份。 |
| Projects `crates/flow/src/flow/send.rs` | managed 模式的原 launcher 清除插件 UI 身份及其上下文，保留原 Agent caller / 无 ID report 语义。 |
| Projects `crates/flow/src/keeper.rs` | managed 模式识别独立暂停标记，keeper 和 ticker 自行退出；保留原后台锁、heartbeat、runner 与更新交接逻辑。 |
| Projects `crates/flow/src/flow/sidebar.rs` | managed 模式关闭旧 Sidebar 元数据同步，避免驱动另一插件界面。 |


## HerdrWork 名称迁移

自有 workspace 为 `herdr-work-core`、`herdr-work-engine`、`herdr-work-tui`、`herdr-work`。新增适配接口使用 `herdr_work_*`，托管环境为 `HERDR_WORK_MANAGED`。Projects CLI 接受旧 `workdesk-*` 命令别名及 `WORKDESK_MANAGED`；暂停标记读取旧文件以保持升级语义，恢复时清理两个名称。固定来源 crate、原始业务目录和许可证均保留。
