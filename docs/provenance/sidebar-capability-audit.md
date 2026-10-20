# Sidebar 与 HerdrWork 能力对齐盘点

日期：2026-10-19。产品基线：HerdrWork v0.1.5，提交 `1a34733492bdaf175bbf6d7531b7f2ade47ed629`。来源基线：Sidebar v0.15.1，导入提交 `081ed133f3faa377438b51ce435fe8a87c439593`。本盘点针对已导入版本，不声称包含此后远端新增能力。

## 2026-10-20 默认 Sidebar 对齐更新

默认 `ui` 已直接托管迁入的 Explorer/SCM 原 App，文件树、菜单、搜索、Quick Open、Git 面板、设置、预览和内置/外部编辑均走来源实现；工作台适配仅由 `--standalone` 使用。宿主打开入口改为每标签页自有实例，接回自动打开、严格切换、打开时聚焦、隐藏 snooze、活动恢复、目录记忆和首次字体引导。cwd 跟随仍使用来源 `CwdFollower`，设置与草稿仅保存在 HerdrWork profile。

H02、H04、H05 已接入来源业务及产品生命周期适配；H03 固定采用统一侧栏，H07 takeover 已迁入，不能再按下面历史表判为待定或不迁移。viewer 真正占用布局空间并保护停车进程；与独立插件共存时选取不含受保护 pane 的最大工作子树，关闭按原树恢复。无安全子树则复用来源新标签页预览路径。自动事件和接管共用布局锁，避免停车过程中生成重复侧栏。

保留明确的产品差异：默认宽度 28 列；产品名称、metadata、锁、配置及安装归 HerdrWork；保持统一 Sidebar，不开放来源的独立 Explorer/Git 双插件部署开关；来源自动更新、旧名配置兼容及独立插件安装/卸载不迁移。默认搜索保持来源能力，来源 Replace 行未实现替换执行，实际批量替换由完整工作台提供。不能据此声称与 VS Code 全功能相同。

验证范围及可复现入口见 [默认入口验证](sidebar-source.md#默认入口验证)。本机原生验证不替代 Linux/Windows 验收。以下 v0.1.6 表描述此前工作台贯通阶段，v0.1.5 表保留原始差异，均不用于判定新的默认入口是否可达。

## v0.1.6 贯通范围与验收入口

本轮按 [#29 规格](https://github.com/JTBlink/herdr-work/issues/29) 和 [K1–K7 任务图](../plans/sidebar-alignment.md) 贯通生产调用，复用来源业务实现。下表记录对应代码与可观察验收；发布状态、固定源码和三平台结果以 GitHub Issues / Actions 为准。

| 范围 | 生产复用入口 | 验收断言 |
| --- | --- | --- |
| 文件导航与快捷操作（F04/F05/F09/F10） | 共享 `tree::navigate`、`file_view` / `scm_view`；TUI interaction / mouse / persistence | 左右展开、进入子项和返回父项；首尾/翻页按正文高度；窄栏隐藏动作不可点击；重启恢复目录、选择和固定文档。 |
| 实时搜索与历史（S03–S06/S08、G14/G15） | Engine QuickIndex / shared search / scm::show_args；TUI search / scm | 无 Enter 的实时结果、防抖与过期请求拒绝；命中第80行正确定位；File History 按当前 SCM 文件查完整 patch。 |
| 编辑补齐（E04–E06） | 原 Editor 的观察/保存/重载；Engine EditorSession；TUI editing | 滚轮不被光标绘制重置；clean 自动重载；dirty 不覆盖；冲突继续/重载/明确覆盖；保存等待期间输入保留。 |
| 批量 Git 与多仓库（G02–G05/G07–G12/G18/G19） | 原 Git / BranchPicker / scm_tree / scm_input；Engine work_git；TUI scm / drafts | 文件、目录、整仓和嵌套仓库边界；stash 含 untracked；tracking、安全二次删除及同步；不同仓库草稿不串；Unknown 需证据核对，不自动重试。 |
| 完整预览（P01–P07/P09） | 从 viewer 移动的共享 preview::Doc，来源 viewer 和产品共同调用 | 高亮、行号、换行、选择复制、固定/切换、图片；可选 glow 格式化和 ffmpeg 视频封面；大文本有界、不可用工具有回退。 |
| 自动刷新与紧凑布局（P08/G20） | 单事件循环、合并式 worker refresh、共享 pane 几何 | 外部文件/索引变化无需 r 更新；手动刷新当前内容；旧请求不覆盖新视图；一个标题行，无绝对路径独占顶栏；80列、中文长标题和双侧停靠。 |
| 产品设置及宿主入口（U02–U05/H01） | 产品 preferences / ui_state / activation；共享 FooterZones / activity | 设置字段合并、关闭装饰跳过可选扫描；活动仓库分支/同步；四种自有 action 分配 TTY 或核验后聚焦，编辑器/弹窗不被请求打断。 |

可复现命令为根 `cargo test --workspace --locked`、来源 `reference-ui` 测试、Python 合约和 `scripts/smoke.py`。`scripts/ui_smoke.py` 使用私有 PTY，实际核对预览刷新、120行编辑滚轮、外部变化/保存冲突、Git索引、任务、第80行命中、Quick Open、80列和终端恢复。可选工具实测可显式运行 Engine 的 `installed_preview_tools_render_markdown_and_a_real_video_poster -- --ignored`，它生成临时视频并调用生产预览入口。

该阶段保留的产品选择（默认入口现已接回来源编辑器）：E08 中的自定义外部终端编辑器仍待决定，内置编辑与文档复用已接入；H02–H05 的宿主布局/打开策略/cwd 跟随仍待定；H06–H08 不迁移。没有把这些项、完整九模块目录迁移或 VS Code Git Graph 的完整图形控件算作已完成能力。

## v0.1.5 原始盘点（历史基线）

以下状态与差异保留初始审计原文，用来说明本轮为何拆分任务；它们不是 v0.1.6 候选的当前状态。

### 结论与判定口径

当前已完成一批业务实现迁移和界面接入，**尚未与 Sidebar 全能力对齐**。最明显的缺口是自动刷新、完整预览、搜索结果定位、批量 Git/分支同步、多仓库交互、提交草稿和编辑冲突恢复。

本次逐项阅读来源 README、插件清单、Explorer/SCM 设置与菜单、键盘/鼠标分派、viewer/editor，以及 HerdrWork 的生产调用方。属于静态入口与行为比对，没有把每个差异重新跑一遍原生交互，也没有修改运行代码、独立插件、注册表或用户窗格。上一版 CI 通过证明当时列出的编译与测试，不代表所有来源功能已经开放。

- **已对齐**：生产入口可达，主要业务实现已复用；不表示每个边缘场景都重新人工验收。
- **部分**：主要路径可达，但下表列出的行为尚缺。
- **未对齐**：来源存在实际实现，产品生产入口未接入。
- **待定**：宿主交互或产品布局选择，需要按 HerdrWork 自身语义决定。
- **不迁移**：会复用独立插件的身份、配置或生命周期，或属于明确移除的旧项目兼容。

源码位于 `crates/herdr-work-code`，不等于功能已对齐。`reference-ui` 来源程序能测试、能编译，也不等于默认生产 HerdrWork 已调用它。

## 文件与导航

来源：[Explorer 输入与界面](../../crates/herdr-work-code/src/explorer_app.rs#L1015)、[文件菜单与操作](../../crates/herdr-work-code/src/actions.rs#L34)、[原文件树](../../crates/herdr-work-code/src/tree.rs)。

| 编号 | 能力 | 状态 | 当前差异或说明 |
| --- | --- | --- | --- |
| F01 | Explorer / Search / Git 活动切换 | 已对齐 | 生产活动栏可达，另外增加 Tasks/Review；数字快捷键采用产品自己的映射。 |
| F02 | 文件树展开、文件类型图标、Git 状态装饰 | 已对齐 | 直接复用 Tree、图标、Git 装饰模型及行绘制。 |
| F03 | 上下移动、Enter 展开/折叠及打开 | 已对齐 | 键盘和鼠标均可用。 |
| F04 | 左右展开/折叠、Home/End、PageUp/PageDown | 未对齐 | 产品文件树只有上下及 Enter，未接原树完整导航；见 [输入分派](../../crates/herdr-work-tui/src/actions.rs#L72)。 |
| F05 | 悬浮高亮、文件行/标题栏快捷操作图标 | 部分 | 有选中高亮、⋯ 菜单和设置入口，无来源 hover 状态与对应行内快捷动作；[鼠标分派](../../crates/herdr-work-tui/src/mouse.rs#L12)。 |
| F06 | 新建文件/目录、重命名、删除确认 | 已对齐 | 来源文件操作实际复用，产品另加路径、目录实例和操作前快照保护。 |
| F07 | 复制绝对/相对路径、系统打开、文件管理器显示 | 已对齐 | Explorer 菜单已接入原 actions；剪贴板含原 OSC 52 路径。 |
| F08 | 打开文件夹与手输路径 | 已对齐 | macOS/Windows 使用原选择器，Linux 使用路径输入。 |
| F09 | 从 Explorer 暂存文件或整个目录 | 未对齐 | 原 `Stage Changes` 存在，但产品调用 `menu_entries(..., false)`，菜单屏蔽该项；目录暂存的嵌套仓库保护算法已有，尚未接入。[入口](../../crates/herdr-work-tui/src/menus.rs#L130)。 |
| F10 | 按工作目录保存展开、选中、滚动状态 | 未对齐 | 来源有 root-keyed TreeState；产品 `App::new` 将展开、选择及预览重置为内存默认值。[初始化](../../crates/herdr-work-tui/src/state.rs#L111)。 |
| F11 | 切换工作目录 | 已对齐 | 可手动更换；与自动跟随邻接 pane 的能力分开计，后者见 H05。 |

## 搜索与 Quick Open

来源：[共享搜索算法](../../crates/herdr-work-code/src/search.rs)、[来源实时搜索调度](../../crates/herdr-work-code/src/explorer_app.rs#L2092)。

| 编号 | 能力 | 状态 | 当前差异或说明 |
| --- | --- | --- | --- |
| S01 | Quick Open 文件索引、ignore/隐藏文件规则和模糊排序 | 已对齐 | 直接调用来源 collect_quick_files/quick_matches。 |
| S02 | 内容搜索：大小写、全词、正则、包含/排除 glob | 已对齐 | 生产调用共享 collect_content_hits；有错误、空结果和截断提示。 |
| S03 | Quick Open 边输入边过滤 | 未对齐 | 产品在输入弹窗按 Enter 后才执行查询；来源即时过滤已缓存索引。[提交入口](../../crates/herdr-work-tui/src/actions.rs#L342)。 |
| S04 | 内容搜索实时防抖与查询/结果连续操作 | 部分 | 原界面有 300 ms 防抖、输入与结果焦点切换；产品为提交式弹窗，不实时搜索。 |
| S05 | 打开命中项并定位到具体行 | 未对齐 | SearchRow 保留 line，但 open_search_result 只传文件路径，未消费行号。[打开结果](../../crates/herdr-work-tui/src/search.rs#L38)。 |
| S06 | 鼠标点击筛选开关、搜索详情区与焦点切换 | 部分 | 产品可用 c/w/*/i/o 设置选项；标题区点击统一打开查询输入，未接原复合面板的按钮命中和焦点行为。[鼠标](../../crates/herdr-work-tui/src/mouse.rs#L50)。 |
| S07 | Ctrl+P / Ctrl+F 快捷入口 | 已对齐 | 未编辑文本时可达；编辑器内 Ctrl+F 继续作为文内查找。 |
| S08 | Ctrl+Shift+F / F12 等来源替代入口 | 部分 | 产品只匹配 Ctrl+p/f；F12 未接入，Ctrl+Shift+F 若由终端报告为大写 Char(F) 则不匹配，尚无来源显式的大/小写分派。[快捷入口](../../crates/herdr-work-tui/src/editing.rs#L106)。 |

## 只读预览与编辑器

来源：[viewer 文档加载与显示](../../crates/herdr-work-code/src/viewer.rs#L1234)、[原编辑模型](../../crates/herdr-work-code/src/editor.rs#L213)。

| 编号 | 能力 | 状态 | 当前差异或说明 |
| --- | --- | --- | --- |
| P01 | 普通文本预览与基本上下/滚轮滚动 | 部分 | 已可预览，但只是 read_to_string + Paragraph，未使用原 Doc 预览模型。[加载](../../crates/herdr-work-engine/src/code.rs#L345)、[显示](../../crates/herdr-work-tui/src/view.rs#L77)。 |
| P02 | 只读预览语法高亮、行号及大文本截断显示 | 未对齐 | 来源 viewer 提供高亮、编号和截断提示；产品只读预览没有这些显示能力。当前为严格 UTF-8、4 MiB 上限，原 viewer 为有界文本展示及 lossy 解码。编辑器自身的高亮是另一条已复用路径。 |
| P03 | Markdown 格式化预览及 glow 不可用时回退 | 未对齐 | 来源 load_file/glow_markdown 已存在，默认生产路径未接。 |
| P04 | 图片预览 | 未对齐 | 来源有格式识别、解码限制和终端像素行绘制；产品按文本读取会报错。 |
| P05 | 视频封面预览 | 未对齐 | 来源支持安全查找 ffmpeg 并加载 poster；产品无入口；该功能本身依赖可选 ffmpeg。 |
| P06 | 只读预览拖选、全选、复制与 copy_on_select | 未对齐 | 来源 viewer 有选择模型及复制队列；产品内容区点击仅改焦点，没有接入只读选择/复制。编辑器里的选择/复制已支持。 |
| P07 | 只读预览换行切换、Home/End、翻页及关闭语义 | 部分 | 产品仅实现上下滚动；无 w 换行切换和原只读分页/首尾/关闭操作。 |
| P08 | 磁盘变化后自动刷新文本/Diff | 未对齐 | 来源 viewer 有有界刷新和失效结果检查；产品没有文件观察/周期刷新，按 r 也只刷新代码快照、任务及活动 Git 抽屉，不重新加载当前文本预览。 |
| P09 | 临时预览、双击固定、多文档复用 | 未对齐 | 当前单个 preview_path，没有文档 tab、固定状态或双击处理。可在产品内部实现，不要求复制来源的宿主 pane 接管。 |
| E01 | e 进入编辑、UTF-8/BOM/CRLF、语法高亮 | 已对齐 | 直接使用原 Editor，并保留 1 MiB/5000 行编辑限制和路径校验。 |
| E02 | 编辑器选择、剪切/复制/粘贴、查找及光标导航 | 已对齐 | KeyEvent 和鼠标选择均路由到原编辑器；终端粘贴已启用。 |
| E03 | 保存、未保存内容保护、外部修改防覆盖 | 已对齐 | Ctrl+S、保存/放弃/继续编辑已接入；冲突时保留编辑内容并拒绝覆盖。 |
| E04 | 编辑器滚轮滚动 | 未对齐 | 原 viewer 对滚轮调用 editor.scroll_by；产品直接交给只处理点选/拖选的 Editor::on_mouse，滚轮不会滚动编辑文本。[产品分派](../../crates/herdr-work-tui/src/mouse.rs#L23)。 |
| E05 | 编辑器干净文件自动重载、dirty 外部变化提示 | 未对齐 | 原 viewer 周期调用 Editor::poll_external；产品无该调用，只在保存时发现冲突。 |
| E06 | 保存冲突后重新加载/明确覆盖/取消 | 部分 | 原 viewer 有独立 Conflict 对话；产品仅报错，没有原 reload/overwrite 决策入口，必须手动保留文本并重新打开。 |
| E07 | 撤销/重做 | 已增强 | 本产品在共用 Editor 增加有界历史；属于本次增强，不把它反算为来源 Sidebar 原有验收项。 |
| E08 | 配置自定义终端编辑器、点击进入、复用已开编辑 tab | 未对齐 | 来源 Settings/actions 已有；生产默认构建未接入。应使用产品自己的配置及文档身份，不能读取独立 Sidebar 的 editor-command。 |

## Source Control 与 Git 对象

来源：[Git 原操作](../../crates/herdr-work-code/src/git.rs#L124)、[SCM 原界面](../../crates/herdr-work-code/src/scm_app.rs)、[共享抽屉及引用操作](../../crates/herdr-work-code/src/scm.rs)。

| 编号 | 能力 | 状态 | 当前差异或说明 |
| --- | --- | --- | --- |
| G01 | Staged/Changes、单文件暂存/取消暂存、Diff | 已对齐 | 生产入口可达，Git 写入复用来源方法并重新校验目录、index 和文件内容。 |
| G02 | 整仓暂存/全部取消暂存 | 未对齐 | 原 stage_all/unstage_all 已有；产品仅有当前文件 s/u，不能把按键同名视为全量入口。 |
| G03 | 丢弃单文件/全部未暂存变更 | 未对齐 | 原 discard/ConfirmDiscard/ConfirmDiscardAll 已有；产品菜单无入口，需保留来源具体目标快照和确认语义。 |
| G04 | Changes 一键创建 stash（含 untracked） | 未对齐 | 来源 ChangesHeaderAction::Stash 可实际 stash push --include-untracked；产品只有已存在 stash 的菜单操作。 |
| G05 | SCM 列表/目录树切换、目录折叠与按目录暂存/取消 | 未对齐 | 来源 build_tree_model、stage_under/unstage_under 已有；产品 scm_rows 是平面列表，没有 t 或目录行。 |
| G06 | 提交暂存变更 | 已对齐 | 当前 C/提交按钮 → CommitPlan → 提交提示；继续保留状态校验。 |
| G07 | 常驻提交框、每仓库草稿、失败后保留输入 | 部分 | 能提交，但产品只有临时 Prompt::Commit；submit_prompt 先取走 prompt/input，后台失败不会恢复草稿；未接来源的仓库级消息和光标/持久化。[提交输入](../../crates/herdr-work-tui/src/actions.rs#L342)。 |
| G08 | AI 生成提交信息及无 claude 时文件名回退 | 未对齐 | 来源 suggest.rs、suggest_message 已有；生产未调用。 |
| G09 | 分支选择器：分支列表、创建、远端 tracking | 部分 | Branches 对象菜单可 checkout，但未接 BranchPicker、New branch、Git::checkout_branch 的远端 tracking 路径；仅有通用 checkout 不等于完整分支选择工作流。 |
| G10 | 安全删除分支与未合并分支的二次强删确认 | 部分 | 产品有确认后的 branch -d；没有原 DeleteRefused::NotMerged → 再确认 → -D 的完整路径，不能默认强制删除。 |
| G11 | Sync、ahead/behind、无 upstream 的禁用提示 | 未对齐 | 原 Git::sync 与原 sync 控件存在；产品只有 Remote 对象 Fetch，Fetch 不等于 Sync。CodeSnapshot 没有保留 ahead/behind/upstream 信息。 |
| G12 | 多仓库独立分组、活动仓库和每仓库提交框 | 部分 | discover_all 已复用，但产品把 changes 合并到一份平面列表，只显示一个 branch；提交/抽屉靠最近文件选择推断仓库，没有原多仓库独立操作面板。 |
| G13 | Graph/Commits/Branches/Worktrees/Remotes/Stashes/Tags 查询 | 已对齐 | 共用 Drawer.load 和 Git 查询已接入；这是基本对象浏览，不等于所有高级操作均有入口。 |
| G14 | File History 跟随当前 SCM 文件 | 部分 | 查询已有，但 load_drawer 从 preview_path 取 file，而非当前 SCM 选中文件；先前预览别的文件时可能查错对象，或完全无目标。[上下文](../../crates/herdr-work-tui/src/scm.rs#L17)。 |
| G15 | 历史/提交/stash/tag 的完整 Show Changes | 部分 | 产品 scm_show 只有 git show --stat --format=fuller，没有原 viewer 的 --patch/着色及 File History 路径过滤。[产品](../../crates/herdr-work-engine/src/scm.rs#L156)、[来源](../../crates/herdr-work-code/src/viewer.rs#L1234)。 |
| G16 | 引用对象操作：checkout/merge/cherry-pick/revert/reset | 已对齐 | 来源 reference_menu/reference_operation 已接入，产品增加统一确认和状态检查；分支选择/创建等额外流程另列 G09。 |
| G17 | 已有 stash apply/pop/drop、远端 fetch、删 tag/移除 worktree | 已对齐 | 来源对应对象菜单及参数构造已接入；不把未有来源入口的任意新建 tag/worktree/remote 算成迁移遗漏。 |
| G18 | SCM 文件上下文菜单完整性 | 部分 | 产品可查看 Diff、stage/unstage 和复制相对路径；原菜单的 discard、系统打开、Reveal、绝对路径复制未完整放到 SCM 文件菜单。Explorer 中有同名能力不能替代此处入口。 |
| G19 | Git 对象操作失败后的可恢复交互 | 部分 | 有错误显示和 Unknown 禁重试记录，但 execute_git_plan 任意错误都会记 unknown，相同 plan 随后被拦；现有 CLI 的通用 reconcile 可以列出并显式解决这些记录，但 TUI 没有相应入口或具体恢复指引。来源界面可在明确失败后继续操作，产品应补可核对的恢复交互，不能直接取消 Unknown 保护。[记录](../../crates/herdr-work-engine/src/scm.rs#L127)。 |
| G20 | 聚焦时自动刷新/可选后台监控刷新 | 未对齐 | 来源 Explorer/SCM tick 已实现；产品 Message 只有 Input/Result，没有周期刷新，外部 Agent 的文件和 Git 变化要手动刷新。[事件](../../crates/herdr-work-tui/src/state.rs#L27)、[主循环](../../crates/herdr-work-tui/src/lib.rs#L93)。 |

## 外观、设置和宿主集成

来源：[Explorer 设置](../../crates/herdr-work-code/src/explorer_app.rs#L2394)、[SCM 设置](../../crates/herdr-work-code/src/scm_app.rs#L2178)、[原插件清单归档](sidebar-plugin.original.toml)。产品设置：[UiPreferences](../../crates/herdr-work-engine/src/preferences.rs#L8)、[设置界面](../../crates/herdr-work-tui/src/menus.rs#L54)。

| 编号 | 能力 | 状态 | 当前差异或说明 |
| --- | --- | --- | --- |
| U01 | 宽度、左右停靠、颜色主题、Material/emoji、隐藏文件、快捷键开关 | 已对齐 | 产品自己的 ui.json 持久化；停靠为工作台内部布局，不承诺复刻宿主跨 pane dock。 |
| U02 | Git 装饰开关的性能语义 | 部分 | 当前开关仅影响绘制，refresh 仍查询 status/ignored；原来源可停止装饰轮询。产品需要分离 SCM 状态必需读取与可禁用的 Explorer 装饰扫描。[读取](../../crates/herdr-work-engine/src/code.rs#L170)、[显示](../../crates/herdr-work-tui/src/sidebar_view.rs#L122)。 |
| U03 | 所有视图的 Git footer：分支、同步及显示开关 | 未对齐 | 当前只有 Explorer 底部 branch 文本和 SCM 标题，没有共用分支选择/同步条，也没有对应开关。 |
| U04 | 图标 auto 探测与字体安装引导 | 部分 | 可手选 Material/emoji，但产品默认 emoji，未接来源自动选择及首次 Nerd Font 引导。字体安装属于可选增强，不能自动改变用户或其他插件的字体设置。 |
| U05 | 产品多个实例间设置、目录/抽屉状态和草稿同步 | 未对齐 | 来源会读取共享状态并合并仓库草稿；产品启动时加载设置，尚无产品实例间同步及原 root-keyed UI 状态恢复。只能共享 HerdrWork 自己的数据。 |
| H01 | 宿主直接 show-explorer/show-search/show-git/quick-open 动作 | 未对齐 | 生产 herdr-plugin.toml 没有这些 [[actions]]，CLI ui 也没有对应初始活动参数；需要新增 HerdrWork 自己的动作标识。 |
| H02 | 临时 tab / split-pane 预览的宿主布局 | 待定 | 当前只在工作台内容区预览。若保留宿主集成，应只创建和管理产品自己的预览，不复用 Sidebar token 或关闭它的窗格。 |
| H03 | Unified / separate Explorer 与 Git 窗格 | 待定 | 原为 Sidebar 独立窗格布局策略；本产品坚持一个 TUI 和输入循环，不能直接启用第二套来源 TUI。 |
| H04 | Auto-open、strict toggle、focus-on-open、b hide | 待定 | 属于产品自身打开/聚焦/隐藏策略；现阶段没有来源等价流程，不应转为控制独立 Sidebar。产品 b 当前为绑定 Agent，属于明确键位差异。 |
| H05 | 跟随邻接 pane cwd、手动目录优先、按 workspace 记根目录 | 待定 | 当前是显式工作目录/任务工作目录，没有来源 CwdFollower。若接入应可选、只读获取目标且保护未保存编辑，不以更改其他插件状态实现跟随。 |
| H06 | 来源 Sidebar 安装、自动更新、ensure/redeploy/恢复其窗格 | 不迁移 | 保持两产品独立；HerdrWork 自有发布、安装和事件入口已存在，不复用来源的注册、资料和 updater。 |
| H07 | replace/takeover 暂存或移动其他工作窗格 | 不迁移 | 当前独立性与唯一终端要求下不接管其他插件/Agent 窗格；不能为了表面能力齐全启用该模块。 |
| H08 | 旧 aa/Sidebar/WORKDESK 名称、目录及状态兼容 | 不迁移 | 用户已明确新项目不考虑旧兼容；原始兼容代码只作为来源测试历史，不进入生产路径。 |

## 不应误报为来源成熟能力的项目

- 原搜索界面虽然有 Replace 输入框，但当前来源只保存/显示输入，没有实际文件替换命令；不能把“全局搜索替换”计为 HerdrWork 漏迁的成熟能力。
- 原视频是封面帧，不是视频播放；原 Graph 为 Git 文本图，不是截图中 VS Code Git Graph 的完整图形化控件。
- 新建 tag/worktree/remote、完整 IDE 补全/语言服务等没有在这份来源里核实到可达入口，不能借“所有能力对齐”扩张为重做 IDE。
- 内置编辑模型已增强撤销/重做，不表示独立 Sidebar 已获得此增强；独立插件项目和安装保持原样。

## 建议实施顺序和验收

| 优先级 | 工作包 | 复用位置与可观察验收 |
| --- | --- | --- |
| P1 | 文件/Git/预览自动刷新 | 抽取来源刷新策略，接入现有唯一事件循环和后台队列；外部进程修改后无需 r 即更新；旧目录/旧请求结果不能覆盖新视图；有未保存编辑时不得覆盖缓冲。 |
| P1 | 搜索与历史对象定位修正 | 消费 SearchRow.line；File History 绑定当前 SCM 文件及明确仓库；命中第 80 行应定位第 80 行；切换文件后历史目标同步；历史 Show Changes 包含实际 patch。 |
| P1 | 提交草稿及失败恢复 | 迁入来源仓库级输入/草稿模型；失败后消息、光标和当前仓库保留；将现有 CLI reconcile 接入 TUI，恢复 Unknown 必须有可核对证据与显式入口，不能以自动重试绕过保护。 |
| P1 | Git 文件/目录/整仓操作 | 接原 stage_all/unstage_all/discard/stage_under/unstage_under、Changes stash 创建和 SCM 菜单；真实 Git fixture 验证文件、目录、整仓、rename/untracked 和嵌套仓库范围。 |
| P1 | 分支/同步及多仓库界面 | 复用 BranchPicker、Git 分支方法、sync 和来源 repo 状态；分别创建/切换/tracking/拒绝误强删；两个同名文件的仓库独立显示，操作与草稿不串仓库。 |
| P1 | 编辑器交互补全 | 接滚轮、poll_external、reload 与冲突决策；原生 PTY 验证滚轮、干净文件自动更新、dirty 冲突保留、显式重载/覆盖和取消。 |
| P2 | 完整预览、文档状态和快速导航 | 从 viewer 抽出可复用加载/渲染/选择模型，接入现有工作台；文本高亮/编号/换行/复制、Markdown、图片、可选视频均有代表 fixture；不引入第二终端或 pane 接管。 |
| P2 | 实时搜索、交互细节、持久化和宿主快捷动作 | 复用原搜索调度/QuickIndex、菜单和树/SCM 状态；补实时过滤、分页/左右导航、hover 动作、Git footer、自有快捷动作；设置与恢复仅写产品资料。 |
| 待定 | 自定义编辑器、宿主预览布局、cwd 跟随与打开策略 | 各项保留显式产品决策；如果做，只使用 HerdrWork 自己的配置、ID 和文档身份。不得将独立插件接管功能混入能力补齐。 |

## 后续任务边界

#28 已关闭对应的是当时明确实现并通过 CI 的能力复用与隔离范围，不是“所有 Sidebar 功能已齐全”的验收。完整模块整合仍由 #17 及 #20–#27 跟踪；本清单是新增能力补齐的输入，尚未自动将其标为已实现或已发布。
