---
status: accepted
date: 2026-10-19
---

# 来源代码作为 HerdrWork 内部模块维护


这替代首版“四个产品 crate + 原结构 upstream + 双 workspace/锁文件”的长期组织约束。仅改名为 vendor/components 会继续保留工程和职责耦合，不能完成该目标；一次性重写或按最终目录盲目拆分又会失去行为证据，因此采用先机械迁移、再整理业务接口的两阶段方案。

