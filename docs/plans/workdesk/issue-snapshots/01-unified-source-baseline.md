# [Workdesk][01] 统一源码目录并跑通复用构建基线

GitHub: [[Workdesk][01] 统一源码目录并跑通复用构建基线](https://github.com/JTBlink/herdr-work/issues/2)

命名及验收约定同步快照；实时状态以 GitHub 为准。

## 当前关闭标准（用户确认）

- 对应任务必须有真实实现，复用指定上游业务逻辑，不以空实现或仅改文档替代。
- 对应实现编译通过即可关闭；记录编译命令和结果。macOS 可本机验证；Linux/Windows 使用 GitHub Actions 成功记录（run URL 与源码 SHA）。
- 涉及打包的任务还需生成实际归档并通过内容和 SHA256 校验。只写工作流而未成功构建不视为编译完成。
- 原手动交互、安装、性能和生命周期验证保留为建议，不作为关闭阻塞；任务关闭不等同于所有运行场景已人工验收。
- 产品名称为 Workdesk，主二进制为 herdr-work（Windows 为 herdr-work.exe）；四个自有 crate 为 workdesk-core、workdesk-engine、workdesk-tui、workdesk。GitHub 仓库仍为 JTBlink/herdr-work，上游保留原名和结构。

## Parent

#1 — https://github.com/JTBlink/herdr-work/issues/1

**What to build:** 维护者从干净 checkout 用统一构建入口编译工作台最小终端宿主和 Projects 后端，能查询全部来源版本并看到三个来源的最小绘制验证。

**Blocked by:** None（可先开始）

## 业务源码复用

导入固定版本的 Projects core/flow/board/web/CLI、Sidebar package 和 Reviewr package，保留原 package/lib 名称、业务模块、许可与行为测试；来源代码完整放入 upstream 并保留原结构；首轮只做接入必需的清单/构建适配，不移动来源模块或改写业务逻辑。

实现时列出仍实际调用的来源逻辑、必要适配和保留的上游测试；不能以重新实现同名功能代替复用。

## 实现范围

- 工作台根 workspace 显式列出四个自有 crate：workdesk-core、workdesk-engine、workdesk-tui、workdesk；来源树进入 upstream 并保留原结构、workspace 和锁文件。统一构建入口可复现编译，不依赖本地参考副本。
- 记录固定来源结构与工作台接入后的构建和相关原测试结果、path dependency 与后端各自锁文件；实际验证 Rust 1.97 需求、依赖解析、终端输入类型及局部绘制。
- 最小可运行宿主显示三个来源的验证视图，不运行其原终端循环；退出、异常和 resize 后终端正常恢复。
- Projects 后端可以报告版本并解析 keeper/keeper-work 等内部入口；不启动旧插件或访问真实任务目录。

## 建议验证（不阻塞关闭）

干净环境构建记录、来源与许可检查、原有行为测试、最小终端演示；未通过的平台明确记录，不以修改 rust-version 绕过失败。

以使用者可见行为验证，不仅测试动作转发。未接入的原菜单/副作用保持关闭，不能因复用代码而默认启用。

## 背景

本任务属于复用优先工作台首版；采用四个自有 crate 的工作台 workspace、保留原结构的 upstream、单 TUI、Projects 原后端体系及隔离数据。执行前读取父规格中完整的首版规格、架构、领域术语及测试约定。阻塞关系同时使用 GitHub 原生依赖，全部阻塞项关闭后才能开始。
