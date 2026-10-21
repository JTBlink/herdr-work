# 固定源码与导入映射

用户已确定[产品内部模块整合方向](../architecture/herdr-work-internal-modules.md)：本页记录当前来源位置与实际调用，后续迁至根 workspace 和 `crates/`，来源记录最终进入 `docs/provenance/`。Sidebar 已迁入 `crates/herdr-work-code`，其余来源仍待迁移；见[当前迁移记录](../provenance/sidebar-source.md)。

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
| Projects 完整仓库 | `upstream/herdr-projects/` | 原根 package/CLI、crates/core、crates/flow、crates/board、crates/web、资源和测试 |
| Sidebar package | `crates/herdr-work-code/` | 原算法、资源及测试；独立插件入口移为显式测试特性，原说明/清单归档到 docs/provenance |


## 复用与升级约束

- 原业务代码必须实际编译和调用；保留相应行为测试与 fixture。包装 CLI 不代替源码复用，CLI 后端仍由导入源码构建。
- 目录整理补丁与业务适配补丁分开说明；任务阶段、Git 解析和 Diff 算法不因整理目录而重写。
- 导入时保留各自 LICENSE、版权及资源许可，记录引用范围；本次只确认原仓库根 LICENSE，不代替传递依赖许可检查。
- 每次实现列出来源模块、保留行为、适配点、行为差异和验证结果。上游升级以明确 commit 更新，不自动跟随分支。
- 关闭旧更新器、安装下载器和自动开窗格逻辑，通过工作台统一构建和版本验收。

## HerdrWork 必要适配

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

在迁入的 SCM 绘制上调整密度：单仓库输入保留侧边和底边，复用的提交按钮取消上下装饰行并同步调用方行高。来源消息换行、草稿、提交/同步、确认与鼠标命中逻辑继续复用。Explorer/SCM 活动栏在设置前加入共享 VS Code 按钮，调用已有后台启动器，不涉及独立插件的资料或生命周期。验证记录见 [提交区与工具栏](../verification/compact-scm-and-toolbar.md)。
