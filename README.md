# HerdrWork

面向 AI 编程的 Herdr Agent 工作台，在一个终端界面中连接任务、Agent、文件、Git 和代码审查。

- 管理任务与执行目录，创建、委派、停止任务。
- 浏览文件树和 SCM，查看真实 Diff，暂存、取消暂存和提交。
- 保存行级审查评论，验证 Agent 身份后回填固定评论批次。
- 重启恢复草稿与选择，对结果未知的操作显式核对。
- 独立数据目录 `~/.herdr-work`，可通过 `--home` 或 `HERDR_WORK_HOME` 指定。

## 安装与启动

从 [Releases](https://github.com/JTBlink/herdr-work/releases) 下载对应平台归档及 `.sha256` 文件，校验后解压，在解压目录执行（Python 3.12+）：

```sh
python3 install.py --prefix ~/.local
~/.local/bin/herdr-work --version
~/.local/bin/herdr-work doctor
~/.local/bin/herdr-work ui /path/to/repository
```

Windows 使用 `python install.py --prefix "$env:LOCALAPPDATA/HerdrWork"`，运行安装目录下的 `bin/herdr-work.exe`。macOS 安装器使用新的文件 inode 替换程序并进行本机 ad hoc 签名；发行包未进行 Apple 公证。

| 平台产物 | 能力 |
| --- | --- |
| macOS Apple Silicon | HerdrWork 与固定 Projects 后端 |
| Linux x86_64 | HerdrWork 与固定 Projects 后端 |
| Windows x86_64 | 原生文件、Git、Diff 与评论界面；Projects 后端仍依赖 Unix，不提供原生任务执行 |

在 Herdr 中执行 `herdr-work ui` 默认以 28 列打开 Sidebar（受宿主 10% 下限及半屏上限约束），右侧保留原终端或 Agent；当前标签页已有自有实例会直接聚焦。默认界面直接复用迁入的 Sidebar 文件树、搜索、Git、设置和键鼠处理。`1`、`2`、`3` 分别进入文件、搜索和 Git，`Ctrl+P` 快速打开文件，`m` 或右键打开菜单，齿轮打开设置。图标使用原有固定占位，Git 数量角标紧贴图标右下角。

文件树菜单按打开与定位、新建、Git、路径复制、重命名与删除分组；在文件或目录上按 `Ctrl+右键`，或选中后按 `m`，选择“复制路径”或“复制相对路径”。菜单支持滚轮和方向键，窄侧栏内自动滚动以显示当前选项。关闭预览会返回来源页面；与独立插件共用预览标签页时只关闭自有文档窗格。 删除文件或文件夹时显示确认弹窗，可点击“取消”或“确认删除”；默认选中取消，Tab / 方向键切换、Enter 执行，Esc / N 取消，Y 确认。删除失败会在弹窗中保留原因并允许重试。

打开文件或 Diff 时，viewer 默认接管侧栏旁的工作区域：原 pane 暂存于停车标签页，进程继续运行，Sidebar 保留。关闭 viewer 后恢复原 pane、分栏方向与比例；未保存编辑继续使用来源确认流程。预览、编辑器、固定文档、搜索过滤条件和 Git 面板沿用 Sidebar 的原交互。设置和恢复记录独立保存在 HerdrWork 资料目录的 `sidebar/` 下。与独立 Sidebar 等插件共存时，预览和恢复仅作用于不含独立插件窗格的工作子树；无法安全接管时使用独立预览标签页。预览位置设置不因插件共存被强制改为分栏。HerdrWork 不会在预览或临时停车标签页自动补开侧栏。

新建标签页或工作区时，自动打开的侧栏默认折叠为图标列，保留工作窗格焦点；点击图标或显式打开文件、搜索、Git 等活动时展开。自动打开、严格切换、打开时聚焦、跟随工作目录和预览位置可在设置中调整。`herdr-work activate toggle` 切换侧栏；点击右下角 `«` 将侧栏折叠为一列图标；点击文件、搜索、Git 或 `»` 展开并恢复原宽度，任务图标独立打开看板。折叠内容固定为 4 列（两列图标槽及左右各一列边距），官方 Herdr 0.9.3 仍有 10% 最小比例限制，需配合[宿主精确列宽补丁](docs/patches/herdr/README.md)才能实际缩到 4 列；折叠状态不显示文字。右上角 `×`（折叠后在底部）或 `b` 关闭整个侧栏，关闭后不会被焦点事件立即重开，显式打开可恢复。每个标签页可拥有自己的侧栏，重开沿用上次活动和目录记忆。

设置中的“文件打开方式”可选择“VS Code”：单击文件、Enter、Quick Open 和搜索结果均交给 VS Code，搜索结果保留行号；文件或目录右键菜单也提供“通过 VS Code 打开”。Git 差异和历史继续使用内置查看器。macOS 优先使用 `code` 命令，并支持标准 Applications 目录下的 VS Code；启动失败显示提示。

选择“浏览器（网页）”后，点击或按 Enter 打开 `.html`、`.htm`、`.xhtml` 文件会交给系统默认浏览器，其他文件保持原有打开规则。网页文件的右键菜单也提供“通过浏览器打开”；查看 HTML 源码时，可点击预览标题栏的“浏览器 B”或按大写 `B` 打开网页。该操作使用默认网页浏览器，不受 HTML 文件的编辑器关联影响，Git Diff 和历史仍使用内置查看器。

Diff 预览标题栏提供上一处、下一处差异按钮和差异计数，也可按 `F7` / `Shift+F7` 循环跳转。连续的删除和新增行作为一处差异，长行换行及调整窗口宽度后仍定位到对应差异块；无文本差异时按钮置灰。

Git 侧栏只保留 Graph 提交历史：单击提交展开变更文件，并显示完整提交说明、作者、时间、父提交及统计；点击文件查看此次提交的 Diff，Backspace 返回。双击侧栏提交或文件可固定预览（标签页模式）。

Git 工具栏的图谱按钮打开独立 Git Graph 大图，侧栏列表也可按 `v` 打开；重复打开会聚焦已有大图，不改变文件预览位置设置。大图按列展示拓扑、引用、说明、日期、作者和短 SHA，单击提交在下方显示详情；窄矮窗口通过“返回图谱”切回。点击范围或按 `s` 切换全部引用 / 当前分支，`r` 刷新，`L` 加载更多提交，Tab 切换图谱与详情，`P` 切换合并提交的比较父提交。历史 Diff 始终比较固定提交，不包含工作区修改。

Explorer 和 Git 的设置面板在列表第一项提供“通过 VS Code 打开”入口，点击即可打开当前项目目录，无需修改默认文件打开方式。底部临时提示显示 3 秒后自动消失。

Sidebar 默认自动刷新，失焦时也会更新：外部新增、删除或重命名文件约 1 秒反映到文件树，Git 更改约 1–2 秒反映到 Changes 和装饰。刷新保留选择、展开目录、滚动位置及提交草稿；检查在后台执行，不必按 `r`。大型目录可在启动前设置 `HERDR_WORK_CODE_BACKGROUND_REFRESH=0`，使失焦侧栏暂停被动读取。

设置中的“配置范围”标明“仅 HerdrWork”。自动打开、聚焦、预览、跟随目录、主题等开关均保存在当前 HerdrWork 资料目录中，不读取或修改独立 Sidebar 的配置；独立 Sidebar 仍可能按它自己的设置自动启动。

`herdr-work ui --standalone` 打开包含任务、审查和内部文件预览的完整工作台。在 Herdr 外，普通 `ui` 在当前终端显示 Sidebar；宿主分栏和接管需要 Herdr 连接。完整工作台的搜索支持搜索/替换、大小写/整词/正则匹配、包含/排除条件和保留大小写；替换先预览确认，并检查文件是否发生外部变化。

Agent 定位和发送需要 Herdr 0.9.3+ 提供的宿主连接。独立启动时仍可浏览 Git 和保存评论。关闭界面不会停止已委派的 Agent。

```sh
herdr-work --help                 # 中文命令帮助
herdr-work ui . --initial-view source-control # 在 Herdr 侧栏直接进入 Git
herdr-work ui --standalone .     # 在当前终端打开完整工作台
herdr-work activate quick-open    # 在 Herdr 中打开或聚焦自有 Quick Open
herdr-work review open --help     # 子命令的详细参数
herdr-work scheduler status
herdr-work scheduler pause   # 暂停工作台后端自动调度，不停止 Agent
herdr-work scheduler resume
herdr-work reconcile         # 查看未知操作，核对后可记录证据
```

卸载使用原归档的 `python3 install.py --prefix ~/.local --uninstall`，用户数据保留。安装回滚使用 `python3 install.py --prefix ~/.local --rollback`。

Herdr 插件入口使用稳定 ID `jtblink.herdr-work`。在仓库根目录使用快捷入口安装并链接插件（Herdr 0.9.3+、Python 3.12+）：

```sh
sh herdr-work-plugins.sh install       # Release 编译、打包、安装并链接当前源码
sh herdr-work-plugins.sh --build-only  # 仅编译打包，不需要 Herdr
sh herdr-work-plugins.sh --release     # 官方安装 VERSION 对应的 GitHub 标签
```

脚本支持 macOS Apple Silicon 和 Linux x86_64，可从任意目录通过脚本路径调用。不传参数等同于 `install`；源码模式自动识别当前平台，以 Cargo Release 模式统一编译两个产品程序，打包并校验后将归档和 `.sha256` 保留到仓库 `dist/`；新包验证成功后清理同平台旧包，仅保留最新一份，失败时保留上一份可用归档。默认继续安装到仓库 `bin/` 并执行 `herdr plugin link <仓库目录>`，请保留该目录；`--build-only` 在打包后结束，不安装或修改 Herdr 配置。发行模式执行 `herdr plugin install JTBlink/herdr-work --ref v<VERSION>`，由 Herdr 管理插件副本。插件清单通过 `scripts/plugin_build.py` 准备并校验发行程序；Projects hook 在独立数据目录运行。

安装时会为文件树添加默认快捷键：先按 `Ctrl+B`，松开后按 `F`，打开或聚焦当前标签页的 HerdrWork。若修改了 Herdr 前缀键，则使用自定义前缀再按 `F`。安装器只在键位空闲时添加；已有绑定或用户改动会保留，重复安装不重复添加。配置前备份宿主配置，不修改独立 Sidebar 的绑定。使用 `herdr plugin install JTBlink/herdr-work` 安装时也会执行相同的快捷键配置步骤。


侧栏获得焦点后，按 `1` / `2` / `3` / `4` 打开文件、搜索、Git 或主看板；输入框中的数字仍作为文本。macOS 支持 `Command+P` 快速打开文件、`Command+F` 搜索，保留 `Ctrl+P` / `Ctrl+F`。看板中的 `Option` 就是 Mac 键盘上的 `⌥`（其他平台称为 `Alt`）；例如 `Option+C` 显示或隐藏最近消息。若终端未传递 Option 组合键，可以按 `?`，用方向键选择操作并按 `Enter` 执行。

插件动作提供 Show HerdrWork Explorer、Search、Source Control 和 Quick Open；宿主为自有工作台分配侧边栏终端，已有实例通过身份核验后聚焦。也可在宿主环境执行 `herdr-work activate show-search --root /path/to/repository`，其余动作名为 `show-explorer`、`show-git`、`quick-open`。编辑器或输入弹窗打开时，动作只聚焦并提示保留当前输入；结束输入后可再次选择活动。

## 开发与验证

依赖 Rust 1.99.0、Git、Python 3.12+ 和平台 C/C++ 构建工具。Linux CI 安装编译依赖，Windows 使用 MSVC。构建入口固定 Cargo 锁文件：

```sh
python3 scripts/build.py --target aarch64-apple-darwin
cargo test --workspace --locked
python3 -m unittest discover -s scripts/tests -v
python3 scripts/package.py create --target aarch64-apple-darwin
python3 scripts/smoke.py --archives dist
```

编译前自动清理超过 7 天的增量缓存，打包验证成功后同平台默认只保留最新一份归档。清理范围、预览命令和保留数量见[构建与产物清理](docs/build-and-cleanup.md)。

真实双插件共存验证见 [Sidebar 共存验证](docs/verification/sidebar-coexistence.md)。测试复制 Sidebar 程序到临时目录，用独立配置和宿主会话运行双方自动事件，不修改已安装插件或现有窗格。

将 target 替换为 `x86_64-unknown-linux-gnu` 或 `x86_64-pc-windows-msvc` 可在对应原生平台构建。CI 编译、测试、打包并执行隔离安装验证；Release 工作流只在所有平台成功后发布附件。流程采用 draft 创建、附件校验、最后发布。

Sidebar 的实际实现已迁入 `crates/herdr-work-code`，Projects 的 CLI 库和四个底层 crate 已迁入根 workspace，专属任务后端入口为 `crates/herdr-work-projects`。HerdrWork 与独立安装的 Sidebar、Projects 插件各自维护配置、数据和生命周期。后续模块整理按[内部模块架构](docs/architecture/herdr-work-internal-modules.md)统一工程，实施进度见[任务索引](docs/plans/internal-modules/README.md)，实际复用见[源码来源与补丁](docs/upstream/source-map.md)。GitHub 仓库仍使用 `JTBlink/herdr-work` 地址。

## 来源与许可

独立社区项目，与 Herdr 和来源项目无官方隶属或背书关系。自有代码、来源代码与内部进程运行器保留各自 MIT 许可，安装归档附带原始许可证。

- [Herdr](https://github.com/herdrdev/herdr)：终端工作区与 Agent 会话宿主。
- [Projects](https://github.com/eliasstravik/herdr-projects)：任务、尝试、执行流程与 keeper。
- [Sidebar](https://github.com/alexarthurs/herdr-sidebar)：文件树、Git 状态和写操作。
