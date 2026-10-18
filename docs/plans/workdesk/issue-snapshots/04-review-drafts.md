# [Workdesk][04] 添加行级评论并在切换或重启后恢复

GitHub: [[Workdesk][04] 添加行级评论并在切换或重启后恢复](https://github.com/JTBlink/herdr-work/issues/5)

命名及验收约定同步快照；实时状态以 GitHub 为准。

## 当前关闭标准（用户确认）

- 对应任务必须有真实实现，复用指定上游业务逻辑，不以空实现或仅改文档替代。
- 对应实现编译通过即可关闭；记录编译命令和结果。macOS 可本机验证；Linux/Windows 使用 GitHub Actions 成功记录（run URL 与源码 SHA）。
- 涉及打包的任务还需生成实际归档并通过内容和 SHA256 校验。只写工作流而未成功构建不视为编译完成。
- 原手动交互、安装、性能和生命周期验证保留为建议，不作为关闭阻塞；任务关闭不等同于所有运行场景已人工验收。
- 产品名称为 Workdesk，主二进制为 herdr-work（Windows 为 herdr-work.exe）；四个自有 crate 为 workdesk-core、workdesk-engine、workdesk-tui、workdesk。GitHub 仓库仍为 JTBlink/herdr-work，上游保留原名和结构。

## Parent

#1 — https://github.com/JTBlink/herdr-work/issues/1

**What to build:** 用户对 Diff 行范围写评论，切换执行或重启后回到原审查会话，代码改变时明确看到过期评论。

**Blocked by:** #4（切片 03）

## 业务源码复用


实现时列出仍实际调用的来源逻辑、必要适配和保留的上游测试；不能以重新实现同名功能代替复用。

## 实现范围

- 评论保留 file/side/范围/snippet/revision，并有工作目录内容指纹；不按当前文件路径重新归属。
- 切任务和退出恢复同一 Review 草稿；损坏文件被保留并报告，不能静默清空。
- Agent 修改文件后评论标为 stale，重新确认锚点前不允许自动发送。
- 同一 state root 只有一个可写 TUI；重复打开定位已有实例，身份无法确认时明确报冲突。
- 模态输入不触发全局动作，窄屏可以完成评论与取消。

## 建议验证（不阻塞关闭）

原评论/选择测试，加上切换、重启、同 HEAD 内容改变、损坏恢复及第二实例的行为验证。

以使用者可见行为验证，不仅测试动作转发。未接入的原菜单/副作用保持关闭，不能因复用代码而默认启用。

## 背景

本任务属于复用优先工作台首版；采用四个自有 crate 的工作台 workspace、保留原结构的 upstream、单 TUI、Projects 原后端体系及隔离数据。执行前读取父规格中完整的首版规格、架构、领域术语及测试约定。阻塞关系同时使用 GitHub 原生依赖，全部阻塞项关闭后才能开始。
