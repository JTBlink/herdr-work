# Issue tracker: GitHub

本仓库的规格和任务位于 JTBlink/herdr-work 的 GitHub Issues，使用已认证的 gh CLI。

- 发布前先审阅完整正文；多行正文写入临时文件，以 --body-file 提交，避免 shell 转义改变内容。
- 一个规格对应一个父 issue，每个实施切片一个子 issue；建立 GitHub 原生 sub-issue 关系。
- 阻塞关系使用 GitHub 原生 issue dependencies，以阻塞任务的数据库 ID 建立关系；若确实不支持，则在正文用 Blocked by 引用并记录原因。
- ready-for-agent 表示任务描述充分；只有所有阻塞项关闭后才可开始。任务发布不表示已开始实施。
- 本地 docs/plans 保留设计、发布前草案和 GitHub 链接索引；发布后任务状态以 GitHub 为准，不维护第二套本地任务状态。
- 后续提问和结论写入对应 issue；不擅自关闭或改写父规格。

PRs as a request surface: no.
