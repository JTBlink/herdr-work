<!-- herdr-work-internal-modules:spec -->
# HerdrWork 产品内部模块整合与长期维护

## 用户已确认的方向


新项目不保留旧 WORKDESK_*、旧目录迁移、旧命令/收据/锁/标记及旧插件数据导入。正常安装所有权、原子替换、失败恢复和当前名称回滚仍保留，用户旧文件不自动删除。默认界面采用原 Sidebar 的 Explorer/Source Control 导航与预览布局，Tasks 通过活动栏进入。

本规格是新一轮规划；不改写已关闭的首版父规格 #1 或原 13 项实施任务。本轮只执行用户追加的 M01 旧兼容清理、M02 默认 Explorer 修复；其余为后续实施任务。ready-for-agent 表示描述充分，开始前仍需满足依赖。

## 规格起草时的事实

以下保留规格起草时的基线；当前 Sidebar 与 Projects 已迁入根 workspace，见 [Projects 迁移记录](../../provenance/projects-source.md)；实时任务状态仍以 GitHub Issues 为准。


## 目标模块

- herdr-work-core：共享领域标识与执行身份契约，不做 I/O。
- herdr-work-runtime：原有有界进程、Herdr socket/pipe 传输，接受显式目标和截止/取消。
- herdr-work-tasks：原任务存储、attempt/run、委派/停止、调度、launcher/report 和 keeper。
- herdr-work-code：原 Tree、Git 读写、Diff，以及路径/实例/index/content 前置校验。
- herdr-work-review：原评论格式化、草稿、固定批次、发送/Unknown 和恢复。
- herdr-work-engine：应用用例、工作上下文、执行绑定与跨模块协调。
- herdr-work-tui：唯一终端、活动栏、Explorer/SCM/Review/Tasks、输入和着色。
- herdr-work：薄主 CLI、组装与退出，主命令保持 herdr-work。
- herdr-work-projects：薄后台入口，复用同一 Tasks 实现；TUI 退出不停止既有 Agent。

Core 不依赖 I/O 模块，Runtime 不依赖业务模块，Code 不依赖 Review，业务模块不依赖 Engine/TUI。Engine 前台任务调用 Rust 接口；后台进程保持独立生命周期，不强制本轮合并为一个二进制。Git 与审查的读取策略可在 Code 内保留，不重写算法来去重。

## 迁移顺序

先建立 11 个现有 package、调用图、资源和测试基线，再将四个产品 package 与来源七个 package 平移到根 workspace（含过渡 task-data/flow/board/web）。调整继承字段、features、锁、嵌入资源、测试和构建路径，避免同时升级依赖或重写业务。随后提取 Runtime、内聚 Tasks/Code/Review、剥离看板/Web/旧插件入口，最后形成九个实际生产 crate。

维护期间不创建长期转发壳、第二份运行源码或第二套任务数据；来源 Git 哈希只记录导入基线，后续修改由 HerdrWork 提交与 manifest SHA 追踪。来源记录最终进入 docs/provenance/，原许可证及资源版权保留。

## 总体验收

- [ ] 当前名称策略与默认 Explorer 修复通过本机行为检查及三平台 CI。
- [ ] 全部生产源码在 crates/，一个 workspace、生产锁与 target，构建不引用 upstream/ 或第二个生产 manifest。
- [ ] 最终九个 crate 有实际调用和清晰接口，过渡成员及 Code→Review 等暂态依赖消失。
- [ ] 原任务/attempt/run、caller/report、keeper、Git/Diff 和评论传输实际复用，状态隔离、未知结果与恢复不退化。
- [ ] 私有 PTY 实际经过默认 Explorer、鼠标导航、文件预览、真实 stage/unstage/Diff、创建任务、80 列和正常退出；分别等待 Tasks 与 Files/Git 就绪。
- [ ] 三平台真实编译/测试/归档/隔离安装，版本/源码 SHA/许可/文件集/校验和一致；Windows 明确排除 Unix 任务实现，不冒充原生任务支持。
- [ ] 来源及测试迁移记录完整，不把未运行的原测试或真人 Agent 会话记为通过；GitHub 原生父子与阻塞依赖、默认标签保留。

架构与决策文档：docs/architecture/herdr-work-internal-modules.md、docs/adr/0001-first-party-workspace.md；任务索引：docs/plans/internal-modules/README.md。普通规划与实现不自动触发正式 Release。
