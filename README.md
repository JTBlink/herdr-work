# Workdesk

面向 AI 编程的 Herdr Agent 工作台，在一个终端界面中连接任务、Agent、文件、Git 和代码审查。

- 浏览 Projects 任务与执行目录，创建、委派、停止任务。
- 浏览文件树和 SCM，查看真实 Diff，暂存、取消暂存和提交。
- 保存行级审查评论，验证 Agent 身份后回填固定评论批次。
- 重启恢复草稿与选择，对结果未知的操作显式核对。
- 独立数据目录 `~/.workdesk`，原插件数据需预览后显式导入。

## 安装与启动

从 [Releases](https://github.com/JTBlink/herdr-work/releases) 下载对应平台归档及 `.sha256` 文件，校验后解压，在解压目录执行（Python 3.12+）：

```sh
python3 install.py --prefix ~/.local
~/.local/bin/workdesk --version
~/.local/bin/workdesk doctor
~/.local/bin/workdesk ui /path/to/repository
```

Windows 使用 `python install.py --prefix "$env:LOCALAPPDATA/Workdesk"`，运行安装目录下的 `bin/workdesk.exe`。macOS 安装器使用新的文件 inode 替换程序并进行本机 ad hoc 签名；发行包未进行 Apple 公证。

| 平台产物 | 能力 |
| --- | --- |
| macOS Apple Silicon | Workdesk 与固定 Projects 后端 |
| Linux x86_64 | Workdesk 与固定 Projects 后端 |
| Windows x86_64 | 原生文件、Git、Diff 与评论界面；Projects 后端仍依赖 Unix，不提供原生任务执行 |

Agent 定位和发送需要 Herdr 0.9.3+ 提供的宿主连接。独立启动时仍可浏览 Git 和保存评论。按 `1`–`4` 切换任务、文件、SCM、审查，`j/k` 移动，`Enter` 打开，`r` 刷新，`q` 关闭界面；具体操作见视图标题。关闭界面不会停止已委派的 Agent。

```sh
workdesk --help
workdesk scheduler status
workdesk scheduler pause   # 暂停工作台后端自动调度，不停止 Agent
workdesk scheduler resume
workdesk import /path/to/original-projects-home          # 只读预览
workdesk import /path/to/original-projects-home --apply  # 显式复制并保留原数据
workdesk reconcile         # 查看未知操作，核对后可记录证据
```

卸载使用原归档的 `python3 install.py --prefix ~/.local --uninstall`；用户数据保留。导入回滚使用 `workdesk rollback-import`，导入后数据已改变时会拒绝覆盖新工作。

Herdr 插件入口使用稳定 ID `jtblink.workdesk`。安装依赖 Python 3.12+，插件安装时按版本下载并校验归档；从源码开发可运行 `python3 scripts/plugin.py --from-source` 后 `herdr plugin link .`。插件的原 Projects hook 在独立数据目录运行。

升级重新运行安装器会保留上一份安装，可用 `python3 install.py --prefix ~/.local --rollback` 恢复。

## 开发与验证

依赖 Rust 1.99.0、Git、Python 3.12+ 和平台 C/C++ 构建工具。Linux CI 安装编译依赖，Windows 使用 MSVC。构建入口固定 Cargo 锁文件：

```sh
python3 scripts/build.py --target aarch64-apple-darwin
cargo test --workspace --locked
python3 -m unittest discover -s scripts/tests -v
python3 scripts/package.py create --target aarch64-apple-darwin
python3 scripts/smoke.py --archives dist
```

将 target 替换为 `x86_64-unknown-linux-gnu` 或 `x86_64-pc-windows-msvc` 可在对应原生平台构建。CI 编译、测试、打包并执行隔离安装验证；Release 工作流只在所有平台成功后发布附件。流程参考固定版本 Reviewr 的矩阵与打包方式，采用 draft 创建、附件校验、最后发布。

四个自有 crate 位于 `crates/`；上游源码位于 `upstream/` 并保留原结构。详见[架构](docs/architecture/workdesk-architecture.md)、[源码来源与补丁](docs/upstream/source-map.md)、[任务与验收标准](docs/plans/workdesk/README.md)。GitHub 仓库仍使用 `JTBlink/herdr-work` 地址。

## 来源与许可

独立社区项目，与 Herdr 和来源项目无官方隶属或背书关系。自有代码与三个来源均保留 MIT 许可，安装归档附带原始许可证。

- [Herdr](https://github.com/herdrdev/herdr)：终端工作区与 Agent 会话宿主。
- [Projects](https://github.com/eliasstravik/herdr-projects)：任务、尝试、执行流程与 keeper。
- [Sidebar](https://github.com/alexarthurs/herdr-sidebar)：文件树、Git 状态和写操作。
- [Reviewr](https://github.com/persiyanov/herdr-reviewr)：Diff、高亮、评论格式、宿主传输和有界进程运行。
