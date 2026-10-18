# Workdesk：GitHub 任务索引

父规格：[[Spec] Workdesk：复用优先工作台首版](https://github.com/JTBlink/herdr-work/issues/1)。保留 13 个实施子任务、默认标签与原生依赖。

四个自有 crate 为 `workdesk-core`、`workdesk-engine`、`workdesk-tui`、`workdesk`，位于 `crates/`；固定版本的上游源码位于 `upstream/`，保留原目录和项目名。命令为 `workdesk`，仓库地址仍为 `JTBlink/herdr-work`。

[架构](../../architecture/workdesk-architecture.md) · [首版规格](spec.md) · [工程配置](engineering-setup.md)

GitHub 是唯一实时任务状态来源。以下正文为命名及验收约定同步快照；实际代码、编译结果和 CI 链接回填到对应 Issue。

关闭标准：必须有真实代码并复用原业务逻辑，编译通过可关闭；Linux/Windows 以 GitHub Actions 为准。打包任务另需实际归档及 SHA256 校验。手动场景保留为建议，不阻塞关闭。Windows 当前不包含 Unix Projects 后端，平台能力与产物清单必须明确说明。

| 切片 | GitHub | 任务正文 | 直接阻塞项 |
| --- | --- | --- | --- |
| 01 | [#2](https://github.com/JTBlink/herdr-work/issues/2) | [[Workdesk][01] 统一源码目录并跑通复用构建基线](issue-snapshots/01-unified-source-baseline.md) | 无 |
| 02 | [#3](https://github.com/JTBlink/herdr-work/issues/3) | [[Workdesk][02] 浏览已有任务并定位具体执行 Agent](issue-snapshots/02-task-execution-navigation.md) | #2 |
| 03 | [#4](https://github.com/JTBlink/herdr-work/issues/4) | [[Workdesk][03] 在所选执行中阅读真实 Diff](issue-snapshots/03-execution-diff.md) | #3 |
| 04 | [#5](https://github.com/JTBlink/herdr-work/issues/5) | [[Workdesk][04] 添加行级评论并在切换或重启后恢复](issue-snapshots/04-review-drafts.md) | #4 |
| 05 | [#6](https://github.com/JTBlink/herdr-work/issues/6) | [[Workdesk][05] 将固定评论批次回填到正确 Agent](issue-snapshots/05-bound-review-delivery.md) | #5 |
| 06 | [#7](https://github.com/JTBlink/herdr-work/issues/7) | [[Workdesk][06] 联动文件树、SCM 与审查文件选择](issue-snapshots/06-files-scm-reading.md) | #4 |
| 07 | [#8](https://github.com/JTBlink/herdr-work/issues/8) | [[Workdesk][07] 在明确 worktree 中暂存与取消暂存](issue-snapshots/07-stage-unstage.md) | #7 |
| 08 | [#9](https://github.com/JTBlink/herdr-work/issues/9) | [[Workdesk][08] 提交明确选中的暂存内容](issue-snapshots/08-commit.md) | #8 |
| 09 | [#10](https://github.com/JTBlink/herdr-work/issues/10) | [[Workdesk][09] 创建、委派和停止任务并保留原后台流程](issue-snapshots/09-task-dispatch-keeper.md) | #3 |
| 10 | [#11](https://github.com/JTBlink/herdr-work/issues/11) | [[Workdesk][10] 重启和断线后核对并恢复工作上下文](issue-snapshots/10-reconcile-recovery.md) | #6, #9, #10 |
| 11 | [#12](https://github.com/JTBlink/herdr-work/issues/12) | [[Workdesk][11] 预览并导入原插件数据且可恢复旧入口](issue-snapshots/11-existing-data-import.md) | #11 |
| 12 | [#13](https://github.com/JTBlink/herdr-work/issues/13) | [[Workdesk][12] 构建安装归档并通过 macOS 编译与打包检查](issue-snapshots/12-macos-bundle.md) | #12 |
| 13 | [#14](https://github.com/JTBlink/herdr-work/issues/14) | [[Workdesk][13] Linux/Windows GitHub Actions 编译、打包与 Release 流程](issue-snapshots/13-linux-parity.md) | #2 |

原生依赖保留共 14 条。#14 在 #2 后即可执行跨平台 CI，不等待本机安装任务。标签 `ready-for-agent` 表示任务描述充分，不代表实现或验收完成。
