---
status: accepted
date: 2026-10-31
---

# 外部审查组件退出生产依赖

用户要求移除未使用的外部审查组件、目录与引用，并从 Git 提交历史中清除其目录，授权强制推送历史重写结果。

生产 workspace 继续只构建 `crates/` 下的内部包。Git 仓库发现、命令与状态读写沿用 Code 的客户端；比较视图直接使用现有行差异算法；显式宿主连接复用 Code 的有界 socket / pipe 传输。

仍被任务和 Git 使用的有界进程树执行器机械迁入 Engine 内部模块，保留原截止时间、并发管道排空和进程组 / JobObject 清理。该部分保留 Dmitry Persiyanov 的 MIT 版权，许可证位于 `crates/herdr-work-engine/LICENSE-process-runtime`，归档随附 `licenses/process-runtime.txt`。

Git 历史在独立镜像中重写，原始完整 bundle、远端 ref 清单及改写映射保存在仓库之外。仅更新明确的 main 与受影响标签，推送使用每个 ref 的原始值作为 lease，不删除其他工作项或用户文件。既有发行附件的源码 SHA 属于改写前历史，不能将其当作改写后源码的构建证明。
