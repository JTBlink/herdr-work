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

默认页面是文件树与预览区，左侧为文件、搜索、Git、任务、审查图标，底部齿轮打开设置。图标复用 Sidebar 的 Material/Emoji，默认自动检测字体，可在设置中明确选择。`1`–`4` 分别进入任务、文件、SCM、审查，`5` 进入搜索；`m` 或右键打开菜单，`,` 打开设置，`Tab` 切换焦点，`r` 刷新，`q` 退出。

顶部只保留同层的视图标题和文档标签，完整目录通过菜单查看；底栏提供活动仓库的分支和同步入口。文件树支持左右、首尾及翻页，展开、选中和固定文档按目录恢复；外部文件和 Git 变化自动刷新。

文件页按 `e` 使用内置编辑器，`Ctrl+S` 保存，`Ctrl+Z/Y` 撤销/重做，`Esc` 返回并处理未保存内容；`Ctrl+P` 快速打开文件，`/` 搜索文本。SCM 按 `s/u` 暂存/取消暂存，`C` 提交，并可展开 Graph、Commits、File History、Branches、Worktrees、Remotes、Stashes、Tags；任务页按 `n` 创建任务。

只读预览支持代码高亮、行号、Markdown 和常见图片；可选 `glow` 提供 Markdown 排版，可选 `ffmpeg` 提供视频封面。长文本有界截断。内容区按 `w` 切换换行，首尾键和翻页键导航，拖选或 `Ctrl/Cmd+A` 全选后按 `Ctrl/Cmd+C` 复制；设置中可关闭选区自动复制。按 `p` 或双击文件固定文档，`[` / `]` 切换固定文档，`Esc` 或右上角 `×` 关闭当前预览；编辑时关闭会保留未保存确认与冲突保护。

Agent 定位和发送需要 Herdr 0.9.3+ 提供的宿主连接。独立启动时仍可浏览 Git 和保存评论。关闭界面不会停止已委派的 Agent。

```sh
herdr-work --help                 # 中文命令帮助
herdr-work ui . --initial-view source-control # 直接进入 Git
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
sh install-plugin.sh --release  # 下载 VERSION 对应的发行包，校验后安装
```

脚本支持 macOS Apple Silicon 和 Linux x86_64，可从任意目录通过脚本路径调用。底层入口为 `scripts/plugin.py`，安装完成后自动执行 `herdr plugin link .`；插件链接依赖当前仓库目录，请保留该目录。插件的原 Projects hook 在独立数据目录运行。

插件动作提供 Show HerdrWork Explorer、Search、Source Control 和 Quick Open；宿主为自有工作台分配终端，已有实例通过身份核验后聚焦。也可在宿主环境执行 `herdr-work activate show-search --root /path/to/repository`，其余动作名为 `show-explorer`、`show-git`、`quick-open`。编辑器或输入弹窗打开时，动作只聚焦并提示保留当前输入；结束输入后可再次选择活动。

## 开发与验证

依赖 Rust 1.99.0、Git、Python 3.12+ 和平台 C/C++ 构建工具。Linux CI 安装编译依赖，Windows 使用 MSVC。构建入口固定 Cargo 锁文件：

```sh
python3 scripts/build.py --target aarch64-apple-darwin
cargo test --workspace --locked
python3 -m unittest discover -s scripts/tests -v
python3 scripts/package.py create --target aarch64-apple-darwin
python3 scripts/smoke.py --archives dist
```



## 来源与许可

独立社区项目，与 Herdr 和来源项目无官方隶属或背书关系。自有代码与三个来源均保留 MIT 许可，安装归档附带原始许可证。

- [Herdr](https://github.com/herdrdev/herdr)：终端工作区与 Agent 会话宿主。
- [Projects](https://github.com/eliasstravik/herdr-projects)：任务、尝试、执行流程与 keeper。
- [Sidebar](https://github.com/alexarthurs/herdr-sidebar)：文件树、Git 状态和写操作。
