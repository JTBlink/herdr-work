# [Workdesk][07] 在明确 worktree 中暂存与取消暂存

GitHub: [[Workdesk][07] 在明确 worktree 中暂存与取消暂存](https://github.com/JTBlink/herdr-work/issues/8)

命名及验收约定同步快照；实时状态以 GitHub 为准。

## 当前关闭标准（用户确认）

- 对应任务必须有真实实现，复用指定上游业务逻辑，不以空实现或仅改文档替代。
- 对应实现编译通过即可关闭；记录编译命令和结果。macOS 可本机验证；Linux/Windows 使用 GitHub Actions 成功记录（run URL 与源码 SHA）。
- 涉及打包的任务还需生成实际归档并通过内容和 SHA256 校验。只写工作流而未成功构建不视为编译完成。
- 原手动交互、安装、性能和生命周期验证保留为建议，不作为关闭阻塞；任务关闭不等同于所有运行场景已人工验收。
- 产品名称为 Workdesk，主二进制为 herdr-work（Windows 为 herdr-work.exe）；四个自有 crate 为 workdesk-core、workdesk-engine、workdesk-tui、workdesk。GitHub 仓库仍为 JTBlink/herdr-work，上游保留原名和结构。

## Parent

#1 — https://github.com/JTBlink/herdr-work/issues/1

**What to build:** 用户在 SCM 中选择文件暂存或取消暂存，并在所有相关视图看到结果；操作不会被任务切换改到其他目录。

**Blocked by:** #7（切片 06）

## 业务源码复用

调用 Sidebar 原 stage/unstage 和嵌套仓库保护逻辑；由 Code 模块承接后台执行、协调和刷新。

实现时列出仍实际调用的来源逻辑、必要适配和保留的上游测试；不能以重新实现同名功能代替复用。

## 实现范围

- 执行前固定目标与预期状态，按 worktree 协调；嵌套仓库使用所属根目录，不跨仓库误暂存。
- 外部 Git 修改或锁冲突明确反馈，重新读取后由用户决定；不盲目重试。
- 业务完成按原 operation 记录，用户切任务后仍正确入账；取消不宣称回滚已发生的写入。
- 完成后失效 SCM、树装饰和 Diff；保留未成功项与错误。

## 建议验证（不阻塞关闭）

真实临时 Git 仓库验证 rename、staged/unstaged、嵌套仓库、外部竞争和切任务期间完成；保留原 Git 操作测试。

以使用者可见行为验证，不仅测试动作转发。未接入的原菜单/副作用保持关闭，不能因复用代码而默认启用。

## 背景

本任务属于复用优先工作台首版；采用四个自有 crate 的工作台 workspace、保留原结构的 upstream、单 TUI、Projects 原后端体系及隔离数据。执行前读取父规格中完整的首版规格、架构、领域术语及测试约定。阻塞关系同时使用 GitHub 原生依赖，全部阻塞项关闭后才能开始。
