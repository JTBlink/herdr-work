# [HerdrWork][10] 重启和断线后核对并恢复工作上下文

GitHub: [[HerdrWork][10] 重启和断线后核对并恢复工作上下文](https://github.com/JTBlink/herdr-work/issues/11)

命名及验收约定同步快照；实时状态以 GitHub 为准。

## 当前关闭标准（用户确认）

- 对应任务必须有真实实现，复用指定上游业务逻辑，不以空实现或仅改文档替代。
- 对应实现编译通过即可关闭；记录编译命令和结果。macOS 可本机验证；Linux/Windows 使用 GitHub Actions 成功记录（run URL 与源码 SHA）。
- 涉及打包的任务还需生成实际归档并通过内容和 SHA256 校验。只写工作流而未成功构建不视为编译完成。
- 原手动交互、安装、性能和生命周期验证保留为建议，不作为关闭阻塞；任务关闭不等同于所有运行场景已人工验收。
- 产品名称为 HerdrWork，主二进制为 herdr-work（Windows 为 herdr-work.exe）；四个自有 crate 为 herdr-work-core、herdr-work-engine、herdr-work-tui、herdr-work。GitHub 仓库仍为 JTBlink/herdr-work，上游保留原名和结构。

## Parent

#1 — https://github.com/JTBlink/herdr-work/issues/1

**What to build:** 用户在进程中断或宿主断线后重开工作台，恢复可证明的执行与草稿，并通过核对入口处理无法确定的操作。

**Blocked by:** #6（切片 05）, #9（切片 08）, #10（切片 09）

## 业务源码复用

保留 Projects 原恢复、锁和 keeper handover，以及 Reviewr 的重连/丢事件和输入核对策略；补充跨模块 reconcile 和恢复提示。

实现时列出仍实际调用的来源逻辑、必要适配和保留的上游测试；不能以重新实现同名功能代替复用。

## 实现范围

- 任务、Execution sidecar、Review、Git 缓存各有权威来源；不把多个原子文件当事务或把缓存当当前身份。
- 可证明的委派/提交/回填结果被核对，无法证明时暂停自动操作；业务历史不因重建投影被伪造。
- pane 移动可重解析，terminal 重建、同路径 worktree 重建或身份冲突时要求重绑，不能延用发送权限。
- 恢复过程保持 UI 可响应；旧 worker、重复/乱序事件和丢事件后全量快照不重复执行操作。

## 建议验证（不阻塞关闭）

跨进程故障矩阵：各意图/外部调用/结果落盘窗口注入退出，宿主重连、事件丢失、keeper handover 和状态损坏；前序任务的安全规则须已通过。

以使用者可见行为验证，不仅测试动作转发。未接入的原菜单/副作用保持关闭，不能因复用代码而默认启用。

## 背景

本任务属于复用优先工作台首版；采用四个自有 crate 的工作台 workspace、保留原结构的 upstream、单 TUI、Projects 原后端体系及隔离数据。执行前读取父规格中完整的首版规格、架构、领域术语及测试约定。阻塞关系同时使用 GitHub 原生依赖，全部阻塞项关闭后才能开始。
