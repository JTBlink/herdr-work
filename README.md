# Herdr Work

面向 AI 编程的 Herdr 一体化工作台，串联任务、Agent、代码改动与审查流程。

> 项目处于初始化阶段。已完成固定版本源码调研，尚未实现、构建或验证工作台插件。

## 项目目标

以 Herdr 的工作区、终端和 Agent 会话为基础，整合以下能力：

| 模块 | 拟复用项目 | 工作台职责 |
| --- | --- | --- |
| 任务看板 | Projects | 任务分派、执行状态与工作区关联 |
| 文件与 Git | Sidebar | 文件浏览、改动查看和 Git 操作 |
| 代码审查 | Reviewr | Diff、行级反馈与 Agent 协作 |
| 统一入口 | Herdr Work | 布局、快捷键、模块联动和版本管理 |

## 初步方案

目标采用一个插件清单、多个内部程序，保留模块边界，统一分发与使用入口。

实施前需要处理原插件 ID、动作重名、启动路径、状态隔离、窗格识别、后台生命周期及更新逻辑。
第一阶段面向 macOS / Linux；完整 Windows 支持需要额外适配 Projects。

## 当前进度

- 已核实 Herdr 0.9.4、Projects 1.2.2、Sidebar 0.15.1、Reviewr 0.46.0 的源码和清单。
- 已记录单插件整合的主要改造点与验证范围。
- 待完成插件骨架、模块适配、合包原型及隔离环境验收。

详细证据见 [插件合并可行性调研](docs/research/plugin-merge-feasibility.md)。

## 上游项目

- [Herdr](https://github.com/herdrdev/herdr)
- [Projects](https://github.com/eliasstravik/herdr-projects)
- [Sidebar](https://github.com/alexarthurs/herdr-sidebar)
- [Reviewr](https://github.com/persiyanov/herdr-reviewr)

当前仓库仅包含项目说明与调研文档。后续引入上游代码时，保留各自版权和许可声明，
并随发行产物核对依赖许可证。
