---
status: accepted
date: 2026-10-19
---

# 来源代码作为 HerdrWork 内部模块维护

用户确认 Projects、Sidebar 的业务实现属于产品自身功能，需要长期开发、定制与重构。决定将生产 Rust package 纳入根 workspace 和 `crates/`，按任务、代码、审查和运行设施的职责整理，保留实现与测试的复用关系，原许可证、版权和来源基线可追溯。

这替代首版“四个产品 crate + 原结构 upstream + 双 workspace/锁文件”的长期组织约束。仅改名为 vendor/components 会继续保留工程和职责耦合，不能完成该目标；一次性重写或按最终目录盲目拆分又会失去行为证据，因此采用先机械迁移、再整理业务接口的两阶段方案。

统一源码组织与运行进程分别决定：后台入口继续复用内部 Tasks 实现，以保留 keeper/Agent 独立生命周期；本轮不强制合为单二进制。新项目不维护旧名称、旧配置和旧插件数据兼容，不自动删除个人旧文件。实施细节、过渡成员、验收与任务见[内部模块架构](../architecture/herdr-work-internal-modules.md)。Sidebar 与 Projects 的生产源码已迁入根 workspace；最终模块职责整理仍未完成。
