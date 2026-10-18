# 工程流程配置确认记录

用户已选择 `JTBlink/herdr-work` 的 GitHub Issues。用户已确认默认标签和本篇配置，内容已写入 AGENTS.md 与 docs/agents；此处保留确认时的配置快照。任务内容和依赖见[拆分草案](README.md)。

检测到 triage 技能已安装，已确认保留默认标签。领域文档采用单上下文；后续 ADR 只在需要记录实际取舍时创建，不为流程预建空文件。

## 已追加到 AGENTS.md

```markdown
## Agent skills

### Issue tracker

规格和任务使用 GitHub Issues：JTBlink/herdr-work。详见 docs/agents/issue-tracker.md。

### Triage labels

保留默认五个 triage 标签。详见 docs/agents/triage-labels.md。

### Domain docs

采用单上下文：根 GLOSSARY.md 与按需创建的 docs/adr/。详见 docs/agents/domain.md。
```

## 已创建 docs/agents/issue-tracker.md

```markdown
# Issue tracker: GitHub

本仓库的规格和任务位于 JTBlink/herdr-work 的 GitHub Issues，使用已认证的 gh CLI。

- 发布前先审阅完整正文；多行正文写入临时文件，以 --body-file 提交，避免 shell 转义改变内容。
- 一个规格对应一个父 issue，每个实施切片一个子 issue；建立 GitHub 原生 sub-issue 关系。
- 阻塞关系使用 GitHub 原生 issue dependencies，以阻塞任务的数据库 ID 建立关系；若确实不支持，则在正文用 Blocked by 引用并记录原因。
- ready-for-agent 表示任务描述充分；只有所有阻塞项关闭后才可开始。任务发布不表示已开始实施。
- 本地 docs/plans 保留设计、发布前草案和 GitHub 链接索引；发布后任务状态以 GitHub 为准，不维护第二套本地任务状态。
- 后续提问和结论写入对应 issue；不擅自关闭或改写父规格。

PRs as a request surface: no.
```

## 已创建 docs/agents/triage-labels.md

```markdown
# Triage Labels

| 技能角色 | GitHub 标签 | 含义 |
| --- | --- | --- |
| needs-triage | needs-triage | 待维护者评估 |
| needs-info | needs-info | 等待补充信息 |
| ready-for-agent | ready-for-agent | 规格充分，可由 Agent 承接；仍须满足阻塞依赖 |
| ready-for-human | ready-for-human | 需要人工实施 |
| wontfix | wontfix | 不安排处理 |

发布前检查并仅创建缺失标签，不覆盖已有标签的名称、颜色或说明。
```

## 已创建 docs/agents/domain.md

```markdown
# Domain Docs

本仓库采用单上下文。探索和实施前读取根 GLOSSARY.md；有 docs/adr/ 时读取相关决策。文件尚不存在时继续工作，不创建占位文档。

术语表只定义领域含义，不存放技术规格或实现细节。架构放在 docs/architecture，规格与任务索引放在 docs/plans。只有实际形成难以撤销、需要说明背景且存在取舍的决策时才新增 ADR。

任务和审查使用统一术语；Task、Attempt、Execution、Run 不混用。遇到术语或 ADR 冲突时先指出冲突，不静默改写。
```

## 确认范围

用户已确认默认五个 triage 标签、上述配置及 13 项 GitHub 任务；目录以架构文档中的四个自有 crate/upstream 规范为准。配置与任务发布均不包括提交、推送、应用实现或软件发布。
