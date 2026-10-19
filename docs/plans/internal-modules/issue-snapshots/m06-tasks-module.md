<!-- herdr-work-internal-modules:M06 -->
# [HerdrWork][M06] 将任务流程整合为内部 Tasks 与后台入口

## 目标与范围

将 task-data/task-flow 的任务存储、attempt/run、委派/停止、报告与调度内聚为 herdr-work-tasks。Engine 的前台操作调用 Rust 接口；herdr-work-projects 作为 keeper/launcher/report/hook 的薄后台入口复用同一实现。

## 实现位置

过渡 herdr-work-task-data/flow/projects、Engine tasks.rs、scripts/hook.py，目标 herdr-work-tasks 与 herdr-work-projects。

## 验收标准

- [ ] 创建/查询/委派/停止/调度经产品接口调用真实原状态机，无双重存储或第二套任务真相。
- [ ] 前台不再通过 Projects CLI JSON 进行任务 CRUD；后台入口仍独立于 TUI 生命周期。
- [ ] launcher 在无继承插件环境的子进程保持目录隔离与 caller/report 语义；重复 hook/keeper 并发/暂停/停止行为通过。
- [ ] 任务数据实现不依赖原看板/Web/UI 初始化、自动安装更新或全局 CWD/环境切换；Windows 不声明任务执行支持。
- [ ] 过渡 task-data/task-flow 中保留的实现与测试迁入新位置，不为换名重写任务算法。

## 父规格与依赖

父规格：#17。直接阻塞：#22。

## 共同约定

复用已导入的原实现，保留许可证、版权与来源。macOS 本机验证，Linux/Windows 以同一源码 SHA 的 GitHub CI 为准；归档任务需真实内容与 SHA256 校验。ready-for-agent 仅表示描述充分，仍须先完成阻塞项。旧名称与旧插件数据兼容不在产品范围；不自动删除用户个人文件，不操作用户已有 pane 或启动真实 Agent。
