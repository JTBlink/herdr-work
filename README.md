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

文件树菜单按打开与定位、新建、Git、路径复制、重命名与删除分组；在文件或目录上按 `Ctrl+右键`，或选中后按 `m`，选择“复制路径”或“复制相对路径”。菜单支持滚轮和方向键，窄侧栏内自动滚动以显示当前选项。关闭预览会返回来源页面；与独立插件共用预览标签页时只关闭自有文档窗格。

打开文件或 Diff 时，viewer 默认接管侧栏旁的工作区域：原 pane 暂存于停车标签页，进程继续运行，Sidebar 保留。关闭 viewer 后恢复原 pane、分栏方向与比例；未保存编辑继续使用来源确认流程。预览、编辑器、固定文档、搜索过滤条件和 Git 面板沿用 Sidebar 的原交互。设置和恢复记录独立保存在 HerdrWork 资料目录的 `sidebar/` 下。与独立 Sidebar 等插件共存时，预览和恢复仅作用于不含独立插件窗格的工作子树；无法安全接管时使用独立预览标签页。预览位置设置不因插件共存被强制改为分栏。HerdrWork 不会在预览或临时停车标签页自动补开侧栏。

自动打开、严格切换、打开时聚焦、跟随工作目录和预览位置可在设置中调整。`herdr-work activate toggle` 切换侧栏；`b` 隐藏后不会被焦点事件立即重开，显式打开可恢复。每个标签页可拥有自己的侧栏，重开沿用上次活动和目录记忆。

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
sh install-plugin.sh            # 默认从当前源码构建，需要 Rust 和平台编译工具
sh install-plugin.sh --release  # 官方安装 VERSION 对应的 GitHub 标签
```

脚本支持 macOS Apple Silicon 和 Linux x86_64，可从任意目录通过脚本路径调用。`scripts/plugin.py` 统一调用 Herdr 官方命令：源码模式构建后执行 `herdr plugin link <仓库目录>`；发行模式执行 `herdr plugin install JTBlink/herdr-work --ref v<VERSION>`，由 Herdr 管理插件副本。源码链接依赖当前仓库目录，请保留该目录。插件清单通过 `scripts/plugin_build.py` 准备并校验发行程序；Projects hook 在独立数据目录运行。

安装时会为文件树添加默认快捷键：先按 `Ctrl+B`，松开后按 `F`，打开或聚焦当前标签页的 HerdrWork。若修改了 Herdr 前缀键，则使用自定义前缀再按 `F`。安装器只在键位空闲时添加；已有绑定或用户改动会保留，重复安装不重复添加。配置前备份宿主配置，不修改独立 Sidebar 的绑定。使用 `herdr plugin install JTBlink/herdr-work` 安装时也会执行相同的快捷键配置步骤。

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

真实双插件共存验证见 [Sidebar 共存验证](docs/verification/sidebar-coexistence.md)。测试复制 Sidebar 程序到临时目录，用独立配置和宿主会话运行双方自动事件，不修改已安装插件或现有窗格。



## 来源与许可

独立社区项目，与 Herdr 和来源项目无官方隶属或背书关系。自有代码与三个来源均保留 MIT 许可，安装归档附带原始许可证。

- [Herdr](https://github.com/herdrdev/herdr)：终端工作区与 Agent 会话宿主。
- [Projects](https://github.com/eliasstravik/herdr-projects)：任务、尝试、执行流程与 keeper。
- [Sidebar](https://github.com/alexarthurs/herdr-sidebar)：文件树、Git 状态和写操作。
