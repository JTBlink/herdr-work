# [HerdrWork][05] 将固定评论批次回填到正确 Agent

GitHub: [[HerdrWork][05] 将固定评论批次回填到正确 Agent](https://github.com/JTBlink/herdr-work/issues/6)

命名及验收约定同步快照；实时状态以 GitHub 为准。

## 当前关闭标准（用户确认）

- 对应任务必须有真实实现，复用指定上游业务逻辑，不以空实现或仅改文档替代。
- 对应实现编译通过即可关闭；记录编译命令和结果。macOS 可本机验证；Linux/Windows 使用 GitHub Actions 成功记录（run URL 与源码 SHA）。
- 涉及打包的任务还需生成实际归档并通过内容和 SHA256 校验。只写工作流而未成功构建不视为编译完成。
- 原手动交互、安装、性能和生命周期验证保留为建议，不作为关闭阻塞；任务关闭不等同于所有运行场景已人工验收。
- 产品名称为 HerdrWork，主二进制为 herdr-work（Windows 为 herdr-work.exe）；四个自有 crate 为 herdr-work-core、herdr-work-engine、herdr-work-tui、herdr-work。GitHub 仓库仍为 JTBlink/herdr-work，上游保留原名和结构。

## Parent

#1 — https://github.com/JTBlink/herdr-work/issues/1

**What to build:** 用户从所选执行发送评论，并准确区分回填成功、明确失败和结果未知；不会因切任务、编辑评论或聚焦失败而丢失或重复反馈。

**Blocked by:** #5（切片 04）

## 业务源码复用


实现时列出仍实际调用的来源逻辑、必要适配和保留的上游测试；不能以重新实现同名功能代替复用。

## 实现范围

- 发送前固定 Execution、目标身份、comment IDs/versions 与文本；持久化 prepared/sending 后执行必要校验。
- 绑定失效、范围偏离、多个候选或 stale 评论阻止发送；不以当前焦点作目标。
- 成功仅消费批次内未修改版本，发送期间新增/编辑的评论保留；聚焦失败不转成发送失败。
- 回执丢失或 sending 状态崩溃恢复为 unknown，保留内容、阻止自动重发并提供显式核对流程。
- 说明宿主接受回填与 Agent 执行的区别；真实会话验证回填是否需要人工提交及目标校验竞态限制。
- 移除原 export 记录完整评论正文的日志副作用，仅保留批次、脱敏标识、结果与耗时。

## 建议验证（不阻塞关闭）

复用导出格式和发送错误测试；故障注入验证重复点击、编辑交错、成功后聚焦失败、断线/超时、切任务及发送后崩溃。

以使用者可见行为验证，不仅测试动作转发。未接入的原菜单/副作用保持关闭，不能因复用代码而默认启用。

## 背景

本任务属于复用优先工作台首版；采用四个自有 crate 的工作台 workspace、保留原结构的 upstream、单 TUI、Projects 原后端体系及隔离数据。执行前读取父规格中完整的首版规格、架构、领域术语及测试约定。阻塞关系同时使用 GitHub 原生依赖，全部阻塞项关闭后才能开始。
