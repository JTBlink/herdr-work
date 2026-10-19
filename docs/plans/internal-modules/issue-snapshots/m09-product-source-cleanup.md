<!-- herdr-work-internal-modules:M09 -->
# [HerdrWork][M09] 清理过渡工程并完成来源与产品入口治理

## 目标与范围

按调用图清理任务 board/web 和旧插件 App/安装器/更新器/自动补 pane 路径，形成最终九成员布局。来源与迁移说明进入 docs/provenance，原版权、许可证与测试归属清晰。

## 实现位置

全部过渡模块及原入口、Cargo、herdr-plugin.toml、scripts 和来源文档；不复制一份永久 upstream/vendor/components 运行树。

## 验收标准

- [ ] 最终九个生产 crate 均有实际调用和责任；task-data/flow/board/web 过渡 package 与运行依赖消失。
- [ ] 只发行 herdr-work 与 Unix 的 herdr-work-projects；没有原插件独立入口或自动动态下载来源代码的路径。
- [ ] 生产 crate 的模块/依赖名符合产品职责，无长期 sw-* facade 或原工程 glob re-export 链；许可证和作者来源不因改名删除。
- [ ] 保留/迁移/移除的来源文件、fixture 和资源逐项有理由；不因测试困难删除仍受支持功能的验证。
- [ ] docs/AGENTS/索引/诊断说明一致，sources 仅为来源基线，实际源码 SHA 由 HerdrWork manifest 追踪。

## 父规格与依赖

父规格：#17。直接阻塞：#25。

## 共同约定

复用已导入的原实现，保留许可证、版权与来源。macOS 本机验证，Linux/Windows 以同一源码 SHA 的 GitHub CI 为准；归档任务需真实内容与 SHA256 校验。ready-for-agent 仅表示描述充分，仍须先完成阻塞项。旧名称与旧插件数据兼容不在产品范围；不自动删除用户个人文件，不操作用户已有 pane 或启动真实 Agent。
