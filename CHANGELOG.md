# Changelog

## Unreleased

## 0.1.0 — 2026-10-18

- 首版 Workdesk：任务与执行导航、文件树和 SCM、Diff、评论草稿与固定批次回填。
- 复用固定版本 Projects、Sidebar、Reviewr 源码，保留原结构与许可；自有代码分为四个 crate。
- 增加工作目录实例校验、持久化操作核对、导入备份及中断恢复、调度暂停与恢复。
- 提供 macOS Apple Silicon、Linux x86_64、Windows x86_64 构建、归档校验及安装 smoke 流程。
- Windows 不包含 Unix Projects 后端；macOS 使用安装时 ad hoc 签名，未进行 Apple 公证。
