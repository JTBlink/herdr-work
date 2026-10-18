# [Workdesk][06] 联动文件树、SCM 与审查文件选择

GitHub: [[Workdesk][06] 联动文件树、SCM 与审查文件选择](https://github.com/JTBlink/herdr-work/issues/7)

命名及验收约定同步快照；实时状态以 GitHub 为准。

## 当前关闭标准（用户确认）

- 对应任务必须有真实实现，复用指定上游业务逻辑，不以空实现或仅改文档替代。
- 对应实现编译通过即可关闭；记录编译命令和结果。macOS 可本机验证；Linux/Windows 使用 GitHub Actions 成功记录（run URL 与源码 SHA）。
- 涉及打包的任务还需生成实际归档并通过内容和 SHA256 校验。只写工作流而未成功构建不视为编译完成。
- 原手动交互、安装、性能和生命周期验证保留为建议，不作为关闭阻塞；任务关闭不等同于所有运行场景已人工验收。
- 产品名称为 Workdesk，主二进制为 herdr-work（Windows 为 herdr-work.exe）；四个自有 crate 为 workdesk-core、workdesk-engine、workdesk-tui、workdesk。GitHub 仓库仍为 JTBlink/herdr-work，上游保留原名和结构。

## Parent

#1 — https://github.com/JTBlink/herdr-work/issues/1

**What to build:** 用户浏览所选 worktree 的文件、查看 SCM 状态并打开对应 Diff，三个视图共享明确的文件和目录上下文。

**Blocked by:** #4（切片 03）

## 业务源码复用

直接使用 Sidebar tree/ignore/git/gitdeco/viewer/syntax 及 Explorer/SCM 的局部 state/render；与 Reviewr 阅读投影共用选择和失效通知。

实现时列出仍实际调用的来源逻辑、必要适配和保留的上游测试；不能以重新实现同名功能代替复用。

## 实现范围

- 只读接入完整保留 staged/unstaged、rename 源路径、嵌套仓库和 submodule 的所属根目录。
- 嵌入路径不修改全局 CWD、不自动 ensure pane、不执行原 heartbeat/update；相对路径显式按 root 解析。
- 浏览目录偏离任务执行目录时有可见提示，不把该目录评论默认为原执行反馈。
- 文件变化由一个刷新协调器合并，SCM、装饰和 Review 被一致失效；不新增三套全仓轮询。

## 建议验证（不阻塞关闭）

保留 Sidebar tree/git/parser fixtures，验证嵌套目录、忽略规则、同名文件跨 worktree、外部编辑和扫描次数。

以使用者可见行为验证，不仅测试动作转发。未接入的原菜单/副作用保持关闭，不能因复用代码而默认启用。

## 背景

本任务属于复用优先工作台首版；采用四个自有 crate 的工作台 workspace、保留原结构的 upstream、单 TUI、Projects 原后端体系及隔离数据。执行前读取父规格中完整的首版规格、架构、领域术语及测试约定。阻塞关系同时使用 GitHub 原生依赖，全部阻塞项关闭后才能开始。
