# Code 模块

遵循根 AGENTS.md。此处为 HerdrWork 自有代码，使用根 workspace 与锁文件。复用并维护 Tree、Git、搜索、编辑器和共享呈现；不增加第三方插件接管或安装入口。

默认库构建必须隔离独立插件的配置、生命周期及自动更新。来源界面测试使用 reference-ui 显式特性，生产发布不得启用该特性。文件和 Git 操作的 HerdrWork 身份及状态校验保留在应用用例边界。

来源维护笔记已归档于 docs/provenance，不能把独立插件的构建和发布规则当作本产品规范。
