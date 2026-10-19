<!-- herdr-work-internal-modules:M03 -->
# [HerdrWork][M03] 登记源码调用图、测试和迁移行为基线

## 目标与范围

为三个来源的七个 package 及四个现有产品 package 建立入口/依赖/资源/测试清单。逐项标明实际产品调用、未开放功能、宿主副作用及迁移归属。记录当前产品行为作为随后机械迁移的对照。

## 实现位置

docs/upstream/source-map.md、来源 Cargo/资源/测试及当前产品测试；迁移记录按新架构进入 docs/provenance。

## 验收标准

- [ ] 列出 11 个实际 package、Projects web→board→flow→core 依赖、跨来源调用以及 CARGO_MANIFEST_DIR/include_* 资源。
- [ ] 任务/attempt/run、caller/report、锁/心跳/keeper、Git 指纹/路径/目录实例、Review Unknown 和 UI 行为均有私有 fixture 的基线。
- [ ] 来源测试逐项标注已运行/需迁移/非产品功能/需私有宿主；不得直接连接用户 Herdr 会话、启动真实 Agent 或以跳过制造通过。
- [ ] 来源基线、版权/许可、当前变更、目标模块与资源清单均可追踪，记录测试命令、结果与平台限制。

## 父规格与依赖

父规格：#17。直接阻塞：#18、#19。

## 共同约定

复用已导入的原实现，保留许可证、版权与来源。macOS 本机验证，Linux/Windows 以同一源码 SHA 的 GitHub CI 为准；归档任务需真实内容与 SHA256 校验。ready-for-agent 仅表示描述充分，仍须先完成阻塞项。旧名称与旧插件数据兼容不在产品范围；不自动删除用户个人文件，不操作用户已有 pane 或启动真实 Agent。
