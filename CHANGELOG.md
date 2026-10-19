# Changelog

## Unreleased

## 0.1.4 — 2026-10-19

- 产品、doctor、界面、插件和 Release 统一为 HerdrWork，主命令和归档为 herdr-work；四个自有 crate、目录、类型及任务文档同步更名。
- 默认数据目录改为 ~/.herdr-work，优先读取 HERDR_WORK_HOME；兼容旧 WORKDESK_HOME。首次运行在旧界面/Agent 停止后校验并复制旧资料，保留旧目录，不覆盖已有新资料。
- 后端使用 herdr-work-projects，插件 ID 为 jtblink.herdr-work。旧安装收据、后端入口和托管标记保留升级兼容，回滚恢复原版本的名称。
- 新增产品/默认路径/环境变量优先级、旧数据迁移和旧安装回滚验收；GitHub Issues 同步当前名称并保留标签、状态与依赖。

## 0.1.3 — 2026-10-19

- 补齐主命令、全部子命令和参数的中文帮助，保留原命令语法及 `help`、`-h` 入口，并增加常用示例。
- 修复首屏数据已返回仍显示 Loading 的问题；分别显示任务和文件/Git 的加载、完成及失败状态，任务后端失败不妨碍浏览文件。
- 宽屏任务页改为任务列表与详情，移除重复列表；空任务页说明独立数据目录，并提供创建、文件浏览和旧 Projects 数据导入引导。
- 加入启动完成、后端错误、重试及过期结果的渲染回归测试；macOS/Linux 安装 smoke 使用真实 PTY 验证数据加载、文件预览、Git Diff、界面内创建任务、缩放和退出恢复。

## 0.1.2 — 2026-10-19

- 主二进制统一为 `herdr-work`（Windows 为 `herdr-work.exe`），同步 CLI、插件入口、打包与安装文档；产品与四个 crate 继续使用 Workdesk/workdesk。
- 升级自动移除安装器管理的旧 `workdesk` 命令，回滚恢复旧名称并清理新名称；拒绝覆盖非本项目程序，安装中途失败恢复上一份安装。
- 增加 macOS/Linux 与 Windows 文件名迁移、回滚、同名冲突和安装中断测试；原生安装 smoke 校验完整命令名与版本。

## 0.1.1 — 2026-10-19

- 修复原 Projects Agent 回调脚本丢失 Workdesk 托管标记、home和报告目录的问题，防止回调回到旧插件默认目录或旧安装接管路径。
- 安装 smoke 实际生成原 launcher，再用无继承环境的独立子进程执行，核对全部隔离目录；实际无 ID report 保持 Agent 语义，创建任务核对明确的仓库目录。
- 0.1.0 的标签构建在公开 Release 前停止；保留该标签，不发布有此缺陷的安装包。

## 0.1.0 — 2026-10-18

- 首版 Workdesk：任务与执行导航、文件树和 SCM、Diff、评论草稿与固定批次回填。
- 增加工作目录实例校验、持久化操作核对、导入备份及中断恢复、调度暂停与恢复。
- 提供 macOS Apple Silicon、Linux x86_64、Windows x86_64 构建、归档校验及安装 smoke 流程。
- Windows 不包含 Unix Projects 后端；macOS 使用安装时 ad hoc 签名，未进行 Apple 公证。
