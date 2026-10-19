# HerdrWork 内部模块整合：规格与实施索引

用户已确认来源代码作为产品自身功能长期开发、定制和重构。当前仍是四个产品 crate 与 upstream 来源工程；后续先迁入十一成员过渡 workspace，再整理为九个产品模块。源码迁移尚未实施。

父规格：[17：产品内部模块整合与长期维护](https://github.com/JTBlink/herdr-work/issues/17)。

[架构与验收](../../architecture/herdr-work-internal-modules.md) · [ADR-0001](../../adr/0001-first-party-workspace.md) · [完整规格](spec.md)

GitHub 是唯一实时任务状态来源。M01/M02 是用户追加的本轮代码修复，其余为规划中的后续实施；不自动触发全部迁移或正式 Release。沿用默认 ready-for-agent 标签，开始前必须满足原生阻塞依赖。原首版 #1 与 13 个实施任务保持历史状态。

| 切片 | GitHub | 范围 | 直接阻塞项 |
| --- | --- | --- | --- |
| M01 | [#18](https://github.com/JTBlink/herdr-work/issues/18) | [清理旧项目兼容，只保留当前名称](issue-snapshots/m01-current-names-only.md) | 无 |
| M02 | [#19](https://github.com/JTBlink/herdr-work/issues/19) | [默认工作区采用 Explorer 与 Source Control 布局](issue-snapshots/m02-default-explorer.md) | 无 |
| M03 | [#20](https://github.com/JTBlink/herdr-work/issues/20) | [登记源码调用图、测试和迁移行为基线](issue-snapshots/m03-behavior-baseline.md) | #18、#19 |
| M04 | [#21](https://github.com/JTBlink/herdr-work/issues/21) | [将现有生产 package 迁入根 workspace 与 crates](issue-snapshots/m04-one-workspace.md) | #20 |
| M05 | [#22](https://github.com/JTBlink/herdr-work/issues/22) | [提取宿主传输与有界进程执行模块](issue-snapshots/m05-runtime-module.md) | #21 |
| M06 | [#23](https://github.com/JTBlink/herdr-work/issues/23) | [将任务流程整合为内部 Tasks 与后台入口](issue-snapshots/m06-tasks-module.md) | #22 |
| M07 | [#24](https://github.com/JTBlink/herdr-work/issues/24) | [整合文件树、Git 与 Diff 到内部 Code](issue-snapshots/m07-code-module.md) | #22 |
| M08 | [#25](https://github.com/JTBlink/herdr-work/issues/25) | [内聚审查草稿、批次与发送恢复](issue-snapshots/m08-review-module.md) | #23、#24 |
| M09 | [#26](https://github.com/JTBlink/herdr-work/issues/26) | [清理过渡工程并完成来源与产品入口治理](issue-snapshots/m09-product-source-cleanup.md) | #25 |
| M10 | [#27](https://github.com/JTBlink/herdr-work/issues/27) | [验收统一工程的三平台构建、归档与安装](issue-snapshots/m10-delivery-acceptance.md) | #26 |

共 10 个原生子任务、10 条直接阻塞关系。每项记录实际调用与原行为测试，完成以真实代码及可复现验收为准。规划文档不维护另一套本地任务状态。
