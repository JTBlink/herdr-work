# Changelog

## Unreleased

## 0.1.1 — 2026-10-19

- 修复原 Projects Agent 回调脚本丢失 Workdesk 托管标记、home和报告目录的问题，防止回调回到旧插件默认目录或旧安装接管路径。
- 安装 smoke 实际生成原 launcher，再用无继承环境的独立子进程执行，核对全部隔离目录；实际无 ID report 保持 Agent 语义，创建任务核对明确的仓库目录。
- 0.1.0 的标签构建在公开 Release 前停止；保留该标签，不发布有此缺陷的安装包。

## 0.1.0 — 2026-10-18

- 首版 Workdesk：任务与执行导航、文件树和 SCM、Diff、评论草稿与固定批次回填。
- 增加工作目录实例校验、持久化操作核对、导入备份及中断恢复、调度暂停与恢复。
- 提供 macOS Apple Silicon、Linux x86_64、Windows x86_64 构建、归档校验及安装 smoke 流程。
- Windows 不包含 Unix Projects 后端；macOS 使用安装时 ad hoc 签名，未进行 Apple 公证。
