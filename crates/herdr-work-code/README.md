# HerdrWork Code

HerdrWork 内部文件、Git、搜索、图标和文本编辑模块，由已导入的 Sidebar 源码迁移并持续维护。原算法、资源及测试保留，许可证见 LICENSE，来源记录见 ../../docs/provenance/sidebar-source.md。

生产构建仅使用库；独立插件接管、更新、宿主窗格管理和旧配置模块不进入默认编译。reference-ui 为来源界面测试的显式工具特性，不打入 HerdrWork 发布归档，也不是独立 Sidebar 插件的升级路径。

从根 workspace 构建：cargo build -p herdr-work-code --lib --locked。
