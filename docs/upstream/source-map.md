# 固定源码与导入映射

用户已确定[产品内部模块整合方向](../architecture/herdr-work-internal-modules.md)：本页记录当前来源位置与实际调用，后续迁至根 workspace 和 `crates/`，来源记录最终进入 `docs/provenance/`。Sidebar 已迁入 `crates/herdr-work-code`，Projects 已迁入根 workspace，见 [Sidebar 记录](../provenance/sidebar-source.md)与 [Projects 记录](../provenance/projects-source.md)。

## 当前获取状态

2026-10-18 三个固定 commit 的源码归档已下载到 `reference-plugins/`；根目录、package 版本及 LICENSE 已核对，关键逻辑已重新阅读。源码获取没有安装、注册或启用插件；随后已完整导入 upstream，实际编译与测试记录以 GitHub Issue 和 CI 为准。

| 来源 | 固定 commit | 已下载目录 | 核对版本 |
| --- | --- | --- | --- |
| Projects | `92a2813be9a7c2f8cda737f6b78ed21e8413c3ce` | `reference-plugins/herdr-projects/` | 1.2.2 |
| Sidebar | `081ed133f3faa377438b51ce435fe8a87c439593` | `reference-plugins/herdr-sidebar/` | 0.15.1 |

版本与下载方式见[参考源码](../reference-plugins.md)。归档不包含 `.git`，固定 commit 来自下载 URL 与归档根目录，不冒充已检查 Git HEAD 的 checkout。

## 已实施目录

以下是首版已经实施的目录。后续按内部模块规划调整，其原始布局不再是永久约束；参考副本保持原貌。

| 来源根目录 | 目标目录 | 保留内容 |
| --- | --- | --- |
| Projects | `crates/herdr-work-projects`、`crates/herdr-work-task-{data,flow,board,web}` | 原 CLI 库、core/flow/board/web、资源及测试；旧脚本仅作测试夹具，原说明和清单归档至 docs/provenance/projects |
| Sidebar package | `crates/herdr-work-code/` | 原算法、资源及测试；独立插件入口移为显式测试特性，原说明/清单归档到 docs/provenance |

Code 与 Projects 已作为产品内部 crate 命名和维护。根 workspace 现有十个成员：core、engine、tui、主 CLI、Code、Projects worker 与 task-data/flow/board/web。Projects 原 CLI 库合入现有 worker，原五个 package 已迁入，使用根 Cargo.lock 和 target；旧目录已删除。最终九模块职责整理尚未完成。

## 复用与升级约束

- 原业务代码必须实际编译和调用；保留相应行为测试与 fixture。包装 CLI 不代替源码复用，CLI 后端仍由导入源码构建。
- 目录整理补丁与业务适配补丁分开说明；任务阶段、Git 解析和 Diff 算法不因整理目录而重写。
- 导入时保留各自 LICENSE、版权及资源许可，记录引用范围；本次只确认原仓库根 LICENSE，不代替传递依赖许可检查。
- 每次实现列出来源模块、保留行为、适配点、行为差异和验证结果。上游升级以明确 commit 更新，不自动跟随分支。
- 关闭旧更新器、安装下载器和自动开窗格逻辑，通过工作台统一构建和版本验收。

## HerdrWork 必要适配

2026-10-31 内部运行设施清理：生产包只引用根 workspace 中的 crate；Engine 内部的进程树执行器保留 MIT 版权与原有取消、截止时间及管道排空行为，源码位于 `crates/herdr-work-engine/src/process.rs`，许可证随发行归档保留。显式宿主通信复用 Code 的有界 IPC。决策见 [内部运行设施](../adr/0002-internal-runtime.md)。

2026-10-31 Explorer 删除确认：保留 Sidebar 菜单捕获的删除目标、`actions::delete` 文件 / 目录实现和树刷新，将底栏 y/N 提示替换为独立模态组件。绘制与点击共用几何，默认取消；键鼠不穿透到后台，删除失败后保留原目标和错误。验证见 [删除确认弹窗](../verification/delete-confirmation.md)。

2026-10-30 Diff 导航：复用 Sidebar 的 `diffview` 事件解析、高亮和 `viewer` 输入循环，派生连续增删块的源行起点；新增共享文档导航状态和标题栏按钮，保留原滚动、换行、选择复制及自动刷新。改动不重写 Diff 算法，不接管独立插件；验证见 [Diff 导航](../verification/diff-navigation.md)。

2026-10-22 顶部 Projects 入口：复用 Code 的活动栏与 Projects 的 `board::place::show`，将原 VS Code 按钮改为后台宿主请求。托管看板通过自有插件 `board` 入口分配终端，传递当前仓库与 HerdrWork 资料；跳过独立插件配置同步、安装提示和旧目录提示。已运行看板沿用原聚焦逻辑，设置与文件菜单中的 VS Code 功能保留。验证见 [Projects 入口](../verification/projects-entry.md)。

VS Code 直接设置入口：Explorer 与 SCM 在文件打开方式后增加当前目录打开动作，共用 `vscode::Launcher` 的有界后台任务和过期结果处理；保留原设置数据与内置预览行为。

VS Code 文件打开扩展：继续使用原 Explorer 的文件、Quick Open 和搜索命中入口，新增产品设置循环和独立 `vscode` 启动适配器；Git 差异/历史及显式内置预览保留原 viewer/takeover。Explorer 与 Git 设置显示产品资料范围，宿主自动打开读取同一 `product_settings_path`；测试以独立 Sidebar 的配置环境作为干扰输入，确认产品设置与编辑器配置不跨目录读取或写入。

2026-10-21 补充：复用 Code 的 `actions` 菜单模型及 Explorer 操作，文件菜单采用中文分组，增加现有预览入口；绘制抽至 `context_menu`，按终端列宽计算弹窗，渲染与点击共用滚动视口。viewer 的独立标签页关闭在共存插件下仍返回来源页面，关闭失败保留控制文件并允许重试。来源文件/Git/剪贴板和 takeover 恢复逻辑继续复用，独立插件不参与关闭。

| 来源文件 | 补丁与实际复用 |
| --- | --- |
| Projects `src/cli/mod.rs` | 增加启动迁移前的只读 `herdr-work-snapshot`，调用原 tasks::list；增加调度暂停/恢复控制。managed 模式跳过旧安装接管及旧数据自动导入，禁止原更新入口。其他 add/delegate/stop/reopen 仍执行原命令逻辑。 |
| Projects `crates/core/src/store/files.rs` | managed 模式的 board_env 为 Agent workspace、重启及实际 launcher 传递托管标记、home、data/config/reports；原模式保持原两项环境。Agent 回调不注入插件 UI 身份。 |
| Projects `crates/flow/src/flow/send.rs` | managed 模式的原 launcher 清除插件 UI 身份及其上下文，保留原 Agent caller / 无 ID report 语义。 |
| Projects `crates/flow/src/keeper.rs` | managed 模式识别独立暂停标记，keeper 和 ticker 自行退出；保留原后台锁、heartbeat、runner 与更新交接逻辑。 |
| Projects `crates/flow/src/flow/sidebar.rs` | managed 模式关闭旧 Sidebar 元数据同步，避免驱动另一插件界面。 |
| Code `crates/herdr-work-code/src/editor.rs` | 原 `poll_external` 抽为后台观察与纯状态应用，原入口也调用共享实现；保留原 reload/save、UTF-8/BOM/CRLF、选择及缓冲算法。工作台每次最多排队一个观察，按编辑会话/路径/工作目录和基线拒绝旧结果；保存确认只合并已落盘字节，保留等待期间的新输入。滚轮视口不再在绘制时被光标位置重置。 |


## 当前名称与兼容清理

当前根 workspace 已增加 `herdr-work-code` 与 `herdr-work-projects`。适配接口使用 `herdr_work_*`，托管环境只读取 `HERDR_WORK_MANAGED`。旧命令别名、旧托管变量、旧暂停标记均已移除；profile、Git 写锁、收据和归档只使用 HerdrWork 当前名称，不再支持旧目录或旧插件数据导入。来源 Git 哈希是导入基线，实际改动由本项目提交追踪。

默认 TUI 已改为 Explorer。文件树/SCM 继续执行原 Tree/Git；活动栏选中样式、Git 状态颜色直接复用 Sidebar 的 `ui` 模块。主程序自行维护一个终端和 scoped 布局，不运行原插件 App 或 pane 自动补建。视口几何与鼠标命中使用同一个 layout 模块，预览有独立焦点与滚动，过期文件预览不覆盖当前选择。

## 搜索与历史接入补丁（#30）

Sidebar 固定基线不变。`search::collect_quick_files/quick_matches` 经 Engine 的 `QuickIndex` 缓存供实时输入使用；内容搜索延续原 `explorer_app` 的 300 ms 防抖和单个在途请求策略，产品 epoch/请求号拒绝失效结果。输入、筛选和结果在同一侧栏内操作；命中行号作为加载目标保存，并交给共享 `Doc` 和原 `Editor` 定位。

原 `viewer::load_show` 的 Git 参数构造抽至共享 `scm::show_args`，来源 viewer 和产品 Engine 均调用同一实现，保留 stat、patch、路径过滤、禁用外部 Diff 及颜色；产品绘制调用来源 `ansi::to_lines`。File History 从当前 SCM 文件及其仓库创建快照，详情继续使用该快照的文件目标。没有新增另一套搜索或 patch 算法。

本地 macOS 验证覆盖真实临时 Git 仓库的按文件彩色 patch，以及 App 输入/渲染的实时筛选、防抖、过期查询合并、命中行预览/编辑、鼠标筛选和历史目标。完整 workspace 通过；终端 PTY、跨平台和发布验收由本轮集成任务继续执行。


## 共享只读预览与产品文档（#34）

从同一 Sidebar 基线 `viewer.rs` 原位移动 `Doc`、行布局、文本选择、ANSI/语法高亮、Markdown、图片与视频封面加载及有序复制队列到 `crates/herdr-work-code/src/preview.rs`。来源 viewer 直接导入该模块，并调用同一 `Doc::draw_body`；独立窗格控制、来源状态读取与终端生命周期仍仅在 reference-ui viewer 内。生产模块不导入这些生命周期接口。

产品使用 Engine 路径边界校验后在 worker 加载共享文档；每个上下文最多一个在途文件预览和一个最新目标，同路径旧请求也不能覆盖新内容。自动刷新以读取前的修改时间/大小为基线；未变化的文件与图片跳过加载，变化后沿原 `apply_diff_refresh` 保留源行位置与换行，内容未变还保留选区。原只读文档用于完整 Git Show 的颜色、布局、换行和选择。

机械抽取后增加必要适配：文本读取上限 1 MiB/5000 行，避免原先先整文件读取再截断；glow 与 ffmpeg 都跳过相对 PATH、当前目录及活动工作目录内的程序，输出和等待有界，失败回退到代码/文本或可见媒体提示。复制沿原 native/OSC52 策略与队列顺序，OSC52 只由唯一事件循环输出；自动复制开关来自 HerdrWork 的 ui.json，队列随产品界面关闭而停止。

文档身份为文件路径，临时预览可替换，固定文档可切换和关闭。共享 `ViewState` 仅含一基源行与换行；产品按目录实例保存固定文档身份与该状态，后台恢复不覆盖新打开的文档或 dirty 编辑器，不保存像素、选区或编辑缓冲。未运行、读取或控制独立插件。

## 自有宿主活动入口（#35）

来源 `ensure::Target` 的活动值和快捷键映射移动至纯 `activity::InitialView`，reference-ui 适配器继续调用同一映射。生产不启用来源 ensure、launch 或插件状态目录。HerdrWork 自有 manifest 的四个动作调用 `activate`，在获取资料目录的 UI 独占锁之前分派；新终端仅由自身 `plugin.pane.open` 入口分配，不从后台动作启动 UI 子进程。

复用窗格需同时匹配 HerdrWork 资料目录身份、UI 实例标记和当前二进制的前台 `ui` 命令。只核查自有候选；不对独立插件或身份不明窗格发送输入、关闭、重跑或清理命令。已有实例使用自身 profile 下的有界原子请求文件，唯一事件循环通过 worker 合并轮询；实例号和 epoch 防止旧请求改写新上下文，编辑器及模态输入期间仅聚焦和显示提示。宿主显式初始活动与目录优先于无关的保存执行绑定。

macOS 本地验证使用临时 Unix socket 运行真实 CLI，覆盖资料锁占用、只聚焦已核实自有窗格、错误前台程序拒绝及自有 TTY 分配参数；私有 PTY 覆盖四种初始活动、CLI/环境目录和旧绑定优先级。没有操作用户宿主窗格；真实宿主协议及其他平台仍由集成验收继续验证。

## Sidebar 原组件实际侧栏绘制

将 `scm_app` 的仓库标题、带边框提交框、三行提交按钮和变高行视口机械移动到 `scm_widgets`，来源 App 改为适配调用。HerdrWork 的 `scm_layout` 只适配快照、草稿和可见矩形，侧栏直接使用同一组 widgets，消息框不再简化成单行文字。中文/换行的列宽修正在共享 `wrap_message` 中，两端共用；鼠标按可见行高命中，提交按钮上下装饰行也指向对应仓库。

活动栏的文件、搜索、Git 与设置图标直接调用原 `activity_icons`/`gear_icon` 表；任务、审查沿用同族扩展。未显式选择图标主题时默认 auto，复用原 Nerd Font 探测；明确保存的 Emoji/Material 不会被覆盖，不读取独立 Sidebar 设置或修改终端字体。

## 默认 Sidebar 与自有生命周期补齐（2026-10-20）

本节替代前述“生产不启用 launch/原 App”“不运行 pane 自动补建”的阶段说明。默认入口由 `sidebar_component` 托管迁入的 Explorer/SCM App，`source_sidebar` 提供唯一终端与输入循环；来源 `root_memory`、`fontsetup`、`launch` 的 cwd/事件范围/切换判断，以及 `snooze` 和 takeover 均参与产品构建。独立插件的 ensure、升级及旧目录兼容仍只保留于 reference-ui，不参与产品生命周期。

从原 ensure 机械移出 `request_close` 至 `pane_close`，两入口共用 Ctrl+Q 关闭与草稿保护；产品 Engine 自有 activation 负责 profile/前台程序核验、每标签页实例、自动打开、严格切换和焦点偏好。自动入口与来源 takeover 使用相同 profile 布局锁，UI 实例锁按 socket/pane 隔离；业务存储锁继续独立保留。设置字段及根目录记忆只读取 HerdrWork 的 `sidebar/`。

接管前按独立插件 metadata/前台程序识别受保护 pane，只停车不包含它们的最大自有子树；无安全工作区域时走来源 Tab 预览。viewer 自身分栏，不借独立插件的邻接 pane 分栏。产品收窄列宽改从内侧工作 pane 移动分隔线，来源 resize 适配同一纯几何 helper 并保留原焦点；避免宿主选择外侧插件分隔线。原 takeover journal、身份核验、恢复与失效处理继续复用，相关来源测试保留。

## 提交区密度与 VS Code 工具栏（2026-10-21）

在迁入的 SCM 绘制上调整密度：单仓库输入恢复完整边框，复用的提交按钮取消上下装饰行并同步调用方行高。2026-10-22 修正输入框压缩后的缺边与空框获焦提示消失，减少底部空行补回列表空间；见[提交输入框修复](../verification/scm-input-layout.md)。来源消息换行、草稿、提交/同步、确认与鼠标命中逻辑继续复用。Explorer/SCM 活动栏在设置前加入共享 VS Code 按钮，调用已有后台启动器，不涉及独立插件的资料或生命周期。验证记录见 [提交区与工具栏](../verification/compact-scm-and-toolbar.md)。

## 整列停靠复用布局恢复（2026-10-22）

来源 ensure 的整列修复未接入产品 activation，导致上下分栏时侧栏只停靠到当前 pane。产品接入迁入的 takeover tree/prune、journal、停车与 replay 实现，新增 `takeover/docking` 用例；原预览流程继续使用相同计划准备与恢复逻辑。宿主活动进程通过显式产品资料定位 journal，不配置进程全局资料，也不读取独立插件目录。已有侧栏可在打开与 ensure 时修复，保护工作区分栏比例与进程身份。验证见 [整列侧栏](../verification/full-height-sidebar.md)。


## 侧栏折叠与关闭（2026-10-29）

在默认 `sidebar_component` 中增加产品折叠状态，`sidebar_chrome` 复用现有图标表、活动按钮样式和颜色。`«` 收起文件树/搜索/Git 正文，保留一列活动入口；`»` 或活动图标展开，恢复原终端列宽和原 App 实例。折叠期间不将窄尺寸交给来源 App 的首选宽度修复，心跳、后台刷新和宿主身份继续沿用原实现。`×` 复用来源 hide、提交草稿保存和 snooze，不重新实现关闭流程，不改变独立插件。宿主收窄仍经现有 `resize_plan`/`resize_sidebar`，保留相邻工作窗格与焦点；实际宽度遵循宿主最小分栏限制。验证见 [侧栏折叠与关闭](../verification/sidebar-collapse.md)。


## Git Graph 信息列（2026-10-29）

保留原 Git CLI 查询与 `git --graph` 的拓扑生成，增加全部引用、完整提交 ID、日期、作者与装饰引用字段。新 `git_graph` 模型解析查询结果，渲染分支/标签和自适应信息列；viewer 的 Graph 使用同一只读文档、滚动、查找和提交详情返回流程，列标题固定在正文上方。提交选择使用源行绑定的完整 OID，不从截断文本推测目标；原 drawer 字符串接口继续保留短 SHA 前置格式。见 [Git Graph 验证](../verification/git-graph.md)。

折叠精确列宽改用显式分隔线路径，复用相邻窗格保护，允许恢复计划记录有效的窄分栏比例。附[独立 Herdr 宿主补丁](../patches/herdr/README.md)，不作为产品 Rust 模块构建，不由普通安装自动接管宿主。

## macOS 快捷键适配（2026-10-31）

迁入的 Explorer/SCM 输入路由增加 Command+P/F，复用 Quick Open、搜索和 Projects 请求；列表数字 4 打开主看板，搜索与提交输入保留数字。显式搜索请求可聚焦已打开的查询框，修复从 Git 转入搜索后仍停在结果列表的问题。唯一终端循环协商增强键盘协议，退出时恢复协议栈并独立清理鼠标/粘贴模式。

Projects 键表继续保存和传输规范 ctrl/alt 和弦；HerdrWork 托管看板在 macOS 显示 Option，设置接受 Option/Control 别名。来源参考界面与生成文档保留规范名称，独立插件不参与配置或生命周期操作。检查与限制见 [macOS 快捷键验证](../verification/macos-shortcuts.md)。
