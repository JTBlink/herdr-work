# [Workdesk][02] 浏览已有任务并定位具体执行 Agent

GitHub: [[Workdesk][02] 浏览已有任务并定位具体执行 Agent](https://github.com/JTBlink/herdr-work/issues/3)

命名及验收约定同步快照；实时状态以 GitHub 为准。

## 当前关闭标准（用户确认）

- 对应任务必须有真实实现，复用指定上游业务逻辑，不以空实现或仅改文档替代。
- 对应实现编译通过即可关闭；记录编译命令和结果。macOS 可本机验证；Linux/Windows 使用 GitHub Actions 成功记录（run URL 与源码 SHA）。
- 涉及打包的任务还需生成实际归档并通过内容和 SHA256 校验。只写工作流而未成功构建不视为编译完成。
- 原手动交互、安装、性能和生命周期验证保留为建议，不作为关闭阻塞；任务关闭不等同于所有运行场景已人工验收。
- 名称统一为 Workdesk；四个自有 crate 为 workdesk-core、workdesk-engine、workdesk-tui、workdesk。GitHub 仓库仍为 JTBlink/herdr-work，上游保留原名和结构。

## Parent

#1 — https://github.com/JTBlink/herdr-work/issues/1

**What to build:** 用户在工作台看到已有任务和多个候选尝试，选择一次执行，确认 worktree 并定位对应 Herdr Agent；切换两个任务时不串上下文。

**Blocked by:** #2（切片 01）

## 业务源码复用

使用 Projects 原任务读取、attempt/执行关联及 board state/update/view；用既有 JSON CLI和适配后的只读快照对接，保留原任务阶段。复用 Reviewr 的宿主快照、重连和移动事件处理策略。

实现时列出仍实际调用的来源逻辑、必要适配和保留的上游测试；不能以重新实现同名功能代替复用。

## 实现范围

- 使用隔离测试数据或明确选择的只读源；读取路径不迁移、不补装、不启动 keeper、不改原插件数据。
- 任务、attempt、run 与 Execution 的映射有明确证据；多个候选分别展示，无法唯一关联时标为 unresolved。
- 宿主身份含 endpoint/session/pane/terminal，pane 移动后重新定位，terminal 重建后不复用旧发送权限。
- 选择是一次上下文提交；过期异步读取不覆盖新任务，窄屏和返回工作台时保留选择。
- board effects 经白名单桥接；原 attach/自动开窗格/写任务等未接入动作明确禁用。

## 建议验证（不阻塞关闭）

两个任务、两个 worktree、并行 attempts、pane 移动/重建及读取无副作用测试；复用相应 Projects fixtures。

以使用者可见行为验证，不仅测试动作转发。未接入的原菜单/副作用保持关闭，不能因复用代码而默认启用。

## 背景

本任务属于复用优先工作台首版；采用四个自有 crate 的工作台 workspace、保留原结构的 upstream、单 TUI、Projects 原后端体系及隔离数据。执行前读取父规格中完整的首版规格、架构、领域术语及测试约定。阻塞关系同时使用 GitHub 原生依赖，全部阻塞项关闭后才能开始。
