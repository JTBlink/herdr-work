# [HerdrWork][12] 构建安装归档并通过 macOS 编译与打包检查

GitHub: [[HerdrWork][12] 构建安装归档并通过 macOS 编译与打包检查](https://github.com/JTBlink/herdr-work/issues/13)

命名及验收约定同步快照；实时状态以 GitHub 为准。

## 当前关闭标准（用户确认）

- 对应任务必须有真实实现，复用指定上游业务逻辑，不以空实现或仅改文档替代。
- 对应实现编译通过即可关闭；记录编译命令和结果。macOS 可本机验证；Linux/Windows 使用 GitHub Actions 成功记录（run URL 与源码 SHA）。
- 涉及打包的任务还需生成实际归档并通过内容和 SHA256 校验。只写工作流而未成功构建不视为编译完成。
- 原手动交互、安装、性能和生命周期验证保留为建议，不作为关闭阻塞；任务关闭不等同于所有运行场景已人工验收。
- 产品名称为 HerdrWork，主二进制为 herdr-work（Windows 为 herdr-work.exe）；四个自有 crate 为 herdr-work-core、herdr-work-engine、herdr-work-tui、herdr-work。GitHub 仓库仍为 JTBlink/herdr-work，上游保留原名和结构。

## Parent

#1 — https://github.com/JTBlink/herdr-work/issues/1

**What to build:** 实现 HerdrWork 与 Projects 后端的统一安装归档，在 macOS 编译并完成归档内容和校验和检查；安装/重启/停用等手动验证作为建议。

**Blocked by:** #12（切片 11）

## 业务源码复用

同一根构建产出工作台 UI 与导入的 Projects 后端；保留三个来源的 MIT 文本和版本信息，替换原下载/自更新及 pane 自动补建入口。

实现时列出仍实际调用的来源逻辑、必要适配和保留的上游测试；不能以重新实现同名功能代替复用。

## 实现范围

- 一个工作台 manifest/版本，明确入口、后端定位、依赖、最低宿主版本和验证过的架构；干净 checkout 可复现构建。
- macOS 上首次启动、窄屏、中文输入、实例定位、退出与重启恢复通过；缺 Git/Herdr 有清楚错误。
- 停用/卸载仅管理自有进程，不意外停止其他 Agent；升级中断与后端 keeper 交接后可恢复/回滚。
- 包内版本、来源、许可与校验清单完整；不触发正式 Release 或 Marketplace 收录声明。

## 建议验证（不阻塞关闭）

macOS 隔离配置的安装/升级/禁用验收，实际产物校验与 CI 记录；仅声明实际测试的 CPU 架构。

以使用者可见行为验证，不仅测试动作转发。未接入的原菜单/副作用保持关闭，不能因复用代码而默认启用。

## 背景

本任务属于复用优先工作台首版；采用四个自有 crate 的工作台 workspace、保留原结构的 upstream、单 TUI、Projects 原后端体系及隔离数据。执行前读取父规格中完整的首版规格、架构、领域术语及测试约定。阻塞关系同时使用 GitHub 原生依赖，全部阻塞项关闭后才能开始。
