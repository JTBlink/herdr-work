<!-- herdr-work-internal-modules:M04 -->
# [HerdrWork][M04] 将现有生产 package 迁入根 workspace 与 crates

## 目标与范围

按架构的十一成员过渡映射机械迁移 package，统一 Cargo workspace/继承字段/根锁和 target。更新依赖、资源、测试及 scripts/build.py/package.py/CI 的路径，不同时重写业务或升级依赖。

## 实现位置

根 Cargo.toml/Cargo.lock、来源七个 package、scripts、.github/workflows/build.yml；移动原实现，保留可审查的 Git rename。

## 验收标准

- [ ] cargo metadata 从根列出全部过渡成员；生产代码位于 crates/，不再依赖嵌套 workspace 或第二份生产锁文件。
- [ ] 保留 serde_json 排序/float_roundtrip 和平台 features；允许必要的不同依赖版本并存，锁差异逐项解释。
- [ ] 产品构建和归档只从根 target 获取当前两个可执行文件，原资源和许可证完整。
- [ ] 来源测试 harness/资源随迁并按 M03 分类安全运行，产品行为对照一致。
- [ ] macOS/Linux 运行适用 workspace 检查；Windows 显式排除过渡 Unix-only 成员并验证其余能力，无空实现冒充支持。

## 父规格与依赖

父规格：#17。直接阻塞：#20。

## 共同约定

复用已导入的原实现，保留许可证、版权与来源。macOS 本机验证，Linux/Windows 以同一源码 SHA 的 GitHub CI 为准；归档任务需真实内容与 SHA256 校验。ready-for-agent 仅表示描述充分，仍须先完成阻塞项。旧名称与旧插件数据兼容不在产品范围；不自动删除用户个人文件，不操作用户已有 pane 或启动真实 Agent。
