---
title: Herdr Agent 工作台技术方案
status: proposed
date: 2026-10-18
tags:
  - research
  - herdr
  - architecture
---

# Herdr Agent 工作台技术方案

> 本篇保留前期候选架构与固定版本源码证据。用户随后明确要求复用业务源码逻辑、统一目录并拆分 GitHub 任务；当前具体提案见[复用优先架构](../architecture/workdesk-architecture.md)，规格与拆分见[任务计划](../plans/workdesk/README.md)。下文的 A/B 候选及四个平铺 crate 是早期提案，不作为最新实施目录；未表示已批准或实现。

## 方案定位与结论

目标是以 **任务、Agent 执行、代码上下文、审查反馈** 为连续工作流的工作台。用户选中任务后，能定位其执行 Agent 和 worktree，查看文件与 Diff，把评论发回正确的执行者，再回到任务状态。

本文是供讨论的技术提案，**尚未决定具体重构合并方式，也未开始提取上游源码到工作台模块或实现工作台**。推荐验证“一个工作台 TUI + 提取上游业务逻辑 + 必要的后台执行进程”，并与“宿主多窗格工作台”做同一场景的对照原型后决定。单纯调用三个插件动作，不能验证上述目标。

**已确认**：产品定位为 Herdr Agent 工作台；主要实现语言采用 Rust；先评审技术方案，再决定如何重构与合并。**待决定**：单 TUI 或多窗格、首版能力范围、源码提取方式、后台运行策略及最终发行结构。

前一份[合并可行性调研](plugin-merge-feasibility.md)保留为插件身份、安装与生命周期的历史证据；其中“先统一入口”的建议不作为当前实施路线。Marketplace 的收录条件与产品架构分开处理，不能以可被索引代替工作台可用性验收。

## 工作台能力与职责

以 Herdr 的工作区、终端和 Agent 会话为基础，统一以下能力的工作上下文和操作流程：

| 模块 | 拟复用项目 | 工作台职责 |
| --- | --- | --- |
| 任务看板 | Projects | 任务分派、执行状态与工作区关联 |
| 文件与 Git | Sidebar | 文件浏览、改动查看和 Git 操作 |
| 工作台框架 | Workdesk | 统一上下文、布局、快捷键、模块联动和版本管理 |

首个验证路径为任务 → Agent 执行 → 指定 worktree 的改动 → 审查反馈 → 任务状态更新。
这些是工作台的目标职责，具体源码复用范围与实现方式仍待评审。

## 调研基线

本方案读取三个固定 checkout 的实际 Rust 源码、Cargo 清单和安装逻辑，不代表上游最新版本；未执行上游构建、性能测试或真实 Herdr 会话验收。

完整参考副本已放入本地 `reference-plugins/`，不纳入 Git；固定版本与重新获取方式见[参考插件源码](../reference-plugins.md)。提取后的工作台代码另行正常跟踪。

| 上游 / 固定版本 | 现有技术方案与可复用 seam | 主要重构成本 |
| --- | --- | --- |
| Projects 1.2.2 / `92a2813` | Cargo workspace 分成 core、flow、board、web 和 CLI；board 的 `update(State, Event)` 返回 effects，不读文件、不调用 Herdr。[P1]、[P2] | 任务目前是动态 JSON 对象；任务流程、宿主操作、keeper、自更新不能整体当作纯领域库。[P8]、[P4]、[P7] |
| Sidebar 0.15.1 / `081ed13` | lib 暴露 tree、git、state、viewer 等；Explorer / SCM App 在二进制模块，由 main 切换并运行各自 poll 循环；Git 采用 CLI。[S1]、[S2]、[S3] | App 同时持有焦点、绘制区域、仓库、后台接收器和 pane 控制；切换时重建 App 并保存草稿，不能直接当作可嵌入 widget。[S2]、[S4] |


## 核心问题：工作上下文由谁负责


建议模型如下，均为**新增设计**，不冒充上游现有类型：

| 概念 | 标识与关系 | 必须维持的规则 |
| --- | --- | --- |
| Repository | 内部 `RepoId`，关联 Git common dir | 一个仓库可有多个 worktree；显示名称、远端 URL 不作唯一键 |
| Worktree | `WorktreeId`、`RepoId`、规范化根目录 | 文件树、Git 写入和 Diff 必须显式指定 worktree；嵌套仓库单独识别 |
| Host context | host endpoint / machine + Herdr session + workspace/tab/pane/terminal ID | 同名 pane 可能出现在不同宿主；不能跨 endpoint 或 session 单独使用 pane ID；移动、重启后重新解析 |
| Task / Execution | `TaskId`；每次执行为独立 `ExecutionId` | Task 可重试且有多次执行；绑定 execution、worktree 和 Agent，保留历史 |
| Agent | session 内的 pane/terminal 与状态 | Agent 状态与 Task 阶段分开；不能把“Agent 空闲”等同“任务完成” |
| Review | execution/worktree + 比较基线 + scope | 评论锚定文件、side、行范围和 revision；切换任务不更改已写评论的归属 |
| Workdesk context | 当前 Task、Execution、Worktree、Review、焦点及 `epoch` | 上下文切换一次提交；异步结果携带 epoch，过期结果不能覆盖新视图 |


选择任务可以更新建议的工作上下文；用户显式浏览其他仓库时，要明确显示“任务执行目录”和“当前浏览目录”的差异。审查发送目标来自 execution 的有效绑定，发送前再次校验。多个候选或失效绑定进入选择流程，禁止静默发送到最近获得焦点的 Agent。

## 可选架构与决策门槛

| 选项 | 工作台形态 | 优势 / 代价 | 何时选择 |
| --- | --- | --- | --- |
| A：单 TUI，提取业务逻辑（推荐验证） | 一个工作台程序负责任务导航、文件树、SCM、Diff、评论；Agent 仍在 Herdr 原生终端执行 | 焦点、上下文和草稿一致；需要重构三套 UI 及 I/O 所有权 | 同屏任务与代码协作、统一键盘与鼠标体验是首要目标 |
| B：宿主多窗格工作台 | 控制程序持有统一领域状态，任务、文件、审查视图通过明确协议联动 | 复用终端独立布局、便于渐进替换；跨 pane 焦点、渲染一致性和事件同步更复杂 | 必须保留可独立调整的原生工具窗格，且实测操作连续性足够好 |
| C：原程序子进程过渡 | 原 TUI 暂时独立运行，新增上下文传递和适配 | 早期回归对照容易；原 UI 未提供统一嵌入接口，不能直接合为单屏 | 仅作提取过程的对照和迁移工具，不据此宣布工作台完成 |

本提案倾向 A，并非选定 A。P0 原型必须比较：选任务 → 定位执行者 → 查看该 worktree 改动 → 加评论 → 回发 → 切到另一个任务，评估操作步骤、焦点可预测性、草稿保存与窗口尺寸限制。若 B 更符合实际多终端工作方式，应调整 UI 方案，领域模型仍可保留。

## 推荐方案的内部组织（提案）

一个 Cargo workspace，先按实际职责形成少量模块；是否拆成独立 crate 由依赖方向和编译成本决定，不预先制造一套插件框架。

可供 P0 检验的 Rust crate 切分如下，名称与目录均为拟议值，当前仓库没有这些源码：

| 拟议 crate | 依赖方向 | 放入的职责 |
| --- | --- | --- |
| `workdesk-core` | 不依赖终端和宿主接入 | typed IDs、上下文、任务/执行/审查模型、commands/results |
| `workdesk-engine` | → core | 提取业务流程、Git/Herdr/存储 adapters、后台 effects；先保持 adapters 为内部模块 |
| `workdesk-tui` | → core / engine | 布局、焦点、输入、视图 reducer/render；唯一终端所有者 |
| `herdr-work` | → engine / tui | CLI、依赖注入与进程入口；选用 headless 模式时不初始化终端 |

同一 executable 可用子命令启动 UI 和 headless 模式，但不要求它们处于同一进程。是否拆开 engine 内部 crate、采用哪份 input/wake implementation、统一 MSRV/Cargo.lock，均以原型结果决定。

```text
Workdesk TUI
  ├─ 工作上下文、统一焦点、快捷键、主题、弹层、布局
  ├─ Tasks / Files / SCM / Review 视图状态及纯状态转换
  └─ effects → 后台工作队列 → 带上下文版本的结果

领域模块：任务与执行关联 / 文件与 Git / Diff 与 Review
外部接入：Herdr 连接 / Git 与 forge 子进程 / 版本化存储
可选 headless runtime：任务调度、恢复、后台执行与通知
```

### 一个终端所有者，一条输入分发路径

工作台拥有 raw mode、alternate screen、鼠标捕获、resize 和终端恢复。其余视图只接受已分配的绘制区域、输入和领域快照，返回状态变化及 effects。不能在一个进程内并跑三个原始 `main` / `run`；它们各自管理终端和循环。[P6]、[S2]、[R1]

布局建议：任务/Agent 导航、文件/SCM 区、主要阅读/Diff 区、执行状态和评论入口。窄屏折叠次要区域；模态输入优先于局部快捷键，再到全局导航。Agent 交互继续使用 Herdr 的终端 pane，工作台执行“定位/聚焦已有 Agent”，可与 Agent pane 并排布局。当前没有证据证明插件可获得原生 GUI 或在 Ratatui 内直接嵌入另一 Herdr PTY；本方案不把内嵌 Agent 终端视为已具备能力。

### 小 interface，复杂实现留在模块内部

先以具体类型和函数实现以下 seam；只有存在实际替换实现时才引入 trait，不为每个上游函数写一层转发：

- **任务执行模块**：接受 `TaskCommand`，输出执行结果和 task/execution 投影；隐藏排队、重试、锁和任务阶段，不向 UI 暴露原始 JSON 操作。
- **Herdr 接入模块**：提供会话快照、领域所需事件和明确的 Agent/pane 命令；隐藏 socket 请求、超时、重连与版本差异，不暴露通用 `call(method, json)` 给各视图。
- **Git 模块**：以显式 worktree 为参数读取 status/diff、执行 stage/unstage/commit；写操作按 worktree 串行，完成后失效相关快照。先复用具体能力，避免发明覆盖 Git 全命令的 DSL。
- **存储模块**：明确负责版本检查、读写、迁移和错误报告；界面绘制不读磁盘，视图切换不等于销毁业务会话。

这样，删除工作台领域模块会让上下文解析、目标校验、事件失效与恢复逻辑重新散布到各视图；这才是值得保留的深模块，而非仅把动作名称换一遍的包装层。

### I/O、事件与后台进程的所有权




**共享进程之前必须移除全局 CWD 切换**：Sidebar 改根目录会调用 `set_current_dir` 并重建 App；原先独立程序里的做法在共享进程会影响其他线程的相对路径。应统一改为显式传入 worktree/root，并仅给子进程设置其 `current_dir`；原有 `CwdFollower` 只能作为上下文变化的输入，不能再自行成为全局上下文所有者。[S9]



Projects keeper 不是普通 UI 轮询：它按 data folder 单实例运行，还有 heartbeat 进程、tick 子进程和升级交接；关看板并不停止后台功能。[P4] 若保留“关闭工作台仍调度”的需求，建议把这部分保留为 headless runtime，先兼容现有锁与恢复语义，再决定是否合并进新的执行器。不能为了单 executable 目标把调度绑死在 TUI 生命周期。

建议区分三种关闭操作：关闭工作台视图；停止自动调度但保留已有 Agent；停用插件并停止自有后台进程。是否终止已有执行 Agent 必须显式定义，不能随 pane 关闭或更新隐式发生。

## 三个上游分别如何处理（待评审）

| 上游 | 建议保留 / 提取 | 建议重构 / 替换 | 第一轮暂缓 |
| --- | --- | --- | --- |
| Projects | task 存取、状态推进、队列/重试、执行关联、hook 去重与恢复；参考现有 reducer/effect 分离 | JSON Task 外增 typed 投影；宿主身份与数据目录注入；board 的终端循环和绘制纳入工作台；keeper 接入统一生命周期 | 跨机器编排、Web UI、全部自动化特性是否纳入首版另列功能清单，不能默认为已兼容 |
| Sidebar | tree、ignore、Git status 解析、SCM 命令、搜索和语法着色的可独立逻辑 | Explorer/SCM App 拆除 pane 控制、独立 heartbeat 与更新；统一 worktree、焦点、绘制区域、Git 刷新和草稿；合并重复 Diff/文件预览职责 | 原生文件选择器、图片预览、复杂 SCM drawers 按需要评估，先保文件与 staged/unstaged 主流程 |


## 存储、迁移与统一发行（提案）


建议在工作台 state/config 根目录下按 `tasks/`、`executions/`、`views/`、`reviews/` 分开保存，各文件有 `schema_version`。第一阶段继续复用 Projects 的存储 implementation 和锁，在新命名空间放 UI 与关联索引；不先引入数据库重写所有存储。Review 草稿持久化是新增能力，应明确 revision 失效与恢复提示。

迁移需支持预览、备份、幂等导入和回滚；导入时保留源 ID 映射，原插件不得与新工作台并行写同一份业务数据。绝对目录只保存在本地运行状态，仓库示例、日志和测试 fixture 使用占位数据；错误报告避免输出代码内容、评论正文及凭据。



## 分阶段实施与验收门槛

以下是后续可执行计划，不表示已授权或完成各阶段实现。

| 阶段 | 输出 | 通过标准 |
| --- | --- | --- |
| P0：方案验证 | 同一用户路径的 A/B 交互原型；上游能力取舍表；依赖和工具链试验记录 | 连续演示任务→执行者→worktree→审查→回发；对焦点、窄屏、多个执行尝试作比较；确定 UI 形态及代码来源策略后再批准源码合并 |
| P1：领域与接入切片 | typed context、Herdr adapter、任务/执行只读投影、统一 UI 壳 | 两个任务/两个 worktree 连续切换不串数据；断线和 pane 移动后能恢复；旧 epoch 结果不落入新上下文；UI 调度不等待宿主 I/O |
| P2：首个完整工作闭环 | Files/SCM/Review 的共享数据源；评论发送 effect；真实执行绑定 | 选任务后只操作指定 worktree；staged/unstaged/rename/嵌套仓库行为正确；错误目标禁止发送；发送失败保草稿，成功仅清对应批次；任务、Agent 与审查状态互不混淆 |
| P3：后台与迁移 | headless runtime 生命周期、事件去重、schema、导入/恢复/回滚 | 关 UI 后行为符合选定策略；重复/乱序事件不重复派发；升级中断可恢复；旧数据不被覆盖；原插件与新版本无 pane 认领和自动拉起冲突 |
| P4：正式发行准备 | 最终 manifest、安装器、许可证、平台 CI、用户文档 | macOS/Linux 分别验证安装、重启、禁用、卸载、升级回滚；缺依赖有明确错误；公开仓库与默认分支元数据满足 Marketplace 要求后，再验索引 |

测试优先跨这些 interface 覆盖真实风险：上下文错配、重复执行、过期异步结果、错误发送对象、Git 写入和迁移恢复。复用上游已有测试作为行为基线，移植时保留 fixture 含义；不要以新写的动作路由单测证明业务流程兼容。性能目标应由 P0 测量后设定，当前不声称响应时延或资源指标已达标。

## 评审需要决定的事项

1. 工作台以单 TUI 为主，还是保留宿主可独立布局的任务/文件/审查窗格？先以 P0 路径比较，再定重构深度。
2. 首版是否要求关闭视图后继续自动调度？这决定 headless runtime 的交付范围，而非是否允许 Agent 继续运行。
4. 源码策略采用固定版本提取并维护补丁，还是争取上游导出库接口？默认先做可独立验证的局部提取；不要同时承诺原版全兼容和快速重写。

## 固定版本源码索引

[P1]: https://github.com/eliasstravik/herdr-projects/blob/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce/Cargo.toml#L23-L44
[P2]: https://github.com/eliasstravik/herdr-projects/blob/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce/crates/board/src/board/update/mod.rs#L1-L58
[P3]: https://github.com/eliasstravik/herdr-projects/blob/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce/crates/core/src/store/files.rs#L9-L145
[P4]: https://github.com/eliasstravik/herdr-projects/blob/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce/crates/flow/src/keeper.rs#L1-L108
[P5]: https://github.com/eliasstravik/herdr-projects/blob/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce/crates/flow/src/hook.rs#L43-L90
[P6]: https://github.com/eliasstravik/herdr-projects/blob/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce/crates/board/src/board/run/mod.rs#L42-L113
[P7]: https://github.com/eliasstravik/herdr-projects/blob/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce/crates/flow/src/install.rs#L1-L25
[P8]: https://github.com/eliasstravik/herdr-projects/blob/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce/crates/core/src/store/tasks.rs#L1-L12
[S1]: https://github.com/alexarthurs/herdr-sidebar/blob/081ed133f3faa377438b51ce435fe8a87c439593/plugins/herdr-sidebar/src/lib.rs#L1-L28
[S2]: https://github.com/alexarthurs/herdr-sidebar/blob/081ed133f3faa377438b51ce435fe8a87c439593/plugins/herdr-sidebar/src/main.rs#L506-L598
[S3]: https://github.com/alexarthurs/herdr-sidebar/blob/081ed133f3faa377438b51ce435fe8a87c439593/plugins/herdr-sidebar/src/git.rs#L1-L83
[S4]: https://github.com/alexarthurs/herdr-sidebar/blob/081ed133f3faa377438b51ce435fe8a87c439593/plugins/herdr-sidebar/src/scm_app.rs#L915-L987
[S5]: https://github.com/alexarthurs/herdr-sidebar/blob/081ed133f3faa377438b51ce435fe8a87c439593/plugins/herdr-sidebar/Cargo.toml#L1-L28
[S6]: https://github.com/alexarthurs/herdr-sidebar/blob/081ed133f3faa377438b51ce435fe8a87c439593/plugins/herdr-sidebar/src/state.rs#L402-L507
[S7]: https://github.com/alexarthurs/herdr-sidebar/blob/081ed133f3faa377438b51ce435fe8a87c439593/plugins/herdr-sidebar/src/updates.rs#L1-L100
[S8]: https://github.com/alexarthurs/herdr-sidebar/blob/081ed133f3faa377438b51ce435fe8a87c439593/plugins/herdr-sidebar/src/state.rs#L565-L820
[S9]: https://github.com/alexarthurs/herdr-sidebar/blob/081ed133f3faa377438b51ce435fe8a87c439593/plugins/herdr-sidebar/src/explorer_app.rs#L2898-L2934
