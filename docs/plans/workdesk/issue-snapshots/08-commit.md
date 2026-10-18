# [Workdesk][08] 提交明确选中的暂存内容

GitHub: [[Workdesk][08] 提交明确选中的暂存内容](https://github.com/JTBlink/herdr-work/issues/9)

命名及验收约定同步快照；实时状态以 GitHub 为准。

## 当前关闭标准（用户确认）

- 对应任务必须有真实实现，复用指定上游业务逻辑，不以空实现或仅改文档替代。
- 对应实现编译通过即可关闭；记录编译命令和结果。macOS 可本机验证；Linux/Windows 使用 GitHub Actions 成功记录（run URL 与源码 SHA）。
- 涉及打包的任务还需生成实际归档并通过内容和 SHA256 校验。只写工作流而未成功构建不视为编译完成。
- 原手动交互、安装、性能和生命周期验证保留为建议，不作为关闭阻塞；任务关闭不等同于所有运行场景已人工验收。
- 名称统一为 Workdesk；四个自有 crate 为 workdesk-core、workdesk-engine、workdesk-tui、workdesk。GitHub 仓库仍为 JTBlink/herdr-work，上游保留原名和结构。

## Parent

#1 — https://github.com/JTBlink/herdr-work/issues/1

**What to build:** 用户确认仓库、暂存内容和提交说明后创建提交，能辨别 hook 失败、成功及超时结果不确定，且不会自动重复提交。

**Blocked by:** #8（切片 07）

## 业务源码复用

调用 Sidebar 原提交能力与错误反馈，保留其 Git CLI 语义；新增目标前置条件、operation 结果核对和工作台输入适配。

实现时列出仍实际调用的来源逻辑、必要适配和保留的上游测试；不能以重新实现同名功能代替复用。

## 实现范围

- 提交前显示目标 worktree、HEAD 与暂存摘要；实际执行前复核，变化后要求重新确认。
- 使用 Git 的原锁并配合工作台作用范围协调；影响共享仓库状态时应用仓库级规则。
- hook 失败保留说明与状态；超时/取消后核对 HEAD 和提交结果，不自动再次 commit。
- 成功后刷新相关视图，切任务不丢业务完成；不包含 push、自动合并或额外远端操作。

## 建议验证（不阻塞关闭）

临时仓库中验证成功、pre-commit hook 拒绝、外部修改、慢 hook 与切任务；不对可重复提交行为作不实保证。

以使用者可见行为验证，不仅测试动作转发。未接入的原菜单/副作用保持关闭，不能因复用代码而默认启用。

## 背景

本任务属于复用优先工作台首版；采用四个自有 crate 的工作台 workspace、保留原结构的 upstream、单 TUI、Projects 原后端体系及隔离数据。执行前读取父规格中完整的首版规格、架构、领域术语及测试约定。阻塞关系同时使用 GitHub 原生依赖，全部阻塞项关闭后才能开始。
