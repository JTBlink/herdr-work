# [Workdesk][09] 创建、委派和停止任务并保留原后台流程

GitHub: [[Workdesk][09] 创建、委派和停止任务并保留原后台流程](https://github.com/JTBlink/herdr-work/issues/10)

命名及验收约定同步快照；实时状态以 GitHub 为准。

## 当前关闭标准（用户确认）

- 对应任务必须有真实实现，复用指定上游业务逻辑，不以空实现或仅改文档替代。
- 对应实现编译通过即可关闭；记录编译命令和结果。macOS 可本机验证；Linux/Windows 使用 GitHub Actions 成功记录（run URL 与源码 SHA）。
- 涉及打包的任务还需生成实际归档并通过内容和 SHA256 校验。只写工作流而未成功构建不视为编译完成。
- 原手动交互、安装、性能和生命周期验证保留为建议，不作为关闭阻塞；任务关闭不等同于所有运行场景已人工验收。
- 产品名称为 Workdesk，主二进制为 herdr-work（Windows 为 herdr-work.exe）；四个自有 crate 为 workdesk-core、workdesk-engine、workdesk-tui、workdesk。GitHub 仓库仍为 JTBlink/herdr-work，上游保留原名和结构。

## Parent

#1 — https://github.com/JTBlink/herdr-work/issues/1

**What to build:** 用户从工作台创建或委派任务，看到新的执行和 Agent，并可按原流程停止；关闭 UI 后既定后台功能继续。

**Blocked by:** #3（切片 02）

## 业务源码复用

调用 Projects 原 CLI/actions/delegate/flow/attempts/keeper/runner 和任务锁，保留原状态推进与内部子命令；不建立第二套调度状态机。

实现时列出仍实际调用的来源逻辑、必要适配和保留的上游测试；不能以重新实现同名功能代替复用。

## 实现范围

- 新建/委派/停止均走原任务修改入口，原参数与工作台支持范围明确；重复命令和 hook 不重复启动同次委派。
- 执行前记录意图，成功后关联源任务和宿主；中断或不明确结果先 reconcile，不再次自动委派。
- keeper、keeper-work、runner 和 Agent hooks 都指向随包后端及隔离数据目录，不误启原插件。
- 关闭 UI、停止自动调度、停止执行三种行为分别演示；停止调度不隐式停止已有 Agent。
- 保留并行 attempts 的独立执行信息，首版未开放的配置和动作明确禁用；关闭原更新器与展示副作用。

## 建议验证（不阻塞关闭）

复用委派、hook、锁和 keeper 相关测试；真实隔离会话验证双击、重复事件、UI 关闭、工作子进程失败与后端路径。

以使用者可见行为验证，不仅测试动作转发。未接入的原菜单/副作用保持关闭，不能因复用代码而默认启用。

## 背景

本任务属于复用优先工作台首版；采用四个自有 crate 的工作台 workspace、保留原结构的 upstream、单 TUI、Projects 原后端体系及隔离数据。执行前读取父规格中完整的首版规格、架构、领域术语及测试约定。阻塞关系同时使用 GitHub 原生依赖，全部阻塞项关闭后才能开始。
