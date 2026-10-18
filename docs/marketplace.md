# Marketplace 配置与发布验收

依据：[Herdr 插件规范](https://herdr.dev/zh-cn/docs/plugins/)与[官方收录说明](https://herdr.dev/zh-cn/docs/marketplace/)。核对日期：2026-10-18。

当前项目正在开发。插件发现元数据与运行能力分别验收；符合索引条件不表示工作台已经可用，也不表示已经收录。

## 当前配置

- 工作台清单位于根目录 [`herdr-plugin.toml`](../herdr-plugin.toml)，插件 ID 为 `jtblink.workdesk`，展示名为 `Workdesk`，版本为 `0.1.0`。
- `min_herdr_version = "0.9.3"` 是当前声明的宿主基线；接入运行入口后需按所用 API 重新核对最低版本。
- `platforms` 声明首批目标 `macos`、`linux`；当前尚未完成两平台运行验收。
- 清单描述明确标注 `in development`。当前仅提供发现元数据，尚未声明构建、动作、窗格、启动或事件命令。
- GitHub 仓库必须保留 `herdr-plugin` topic；它是远端仓库设置，不能用 README 关键词或本地配置代替。

2026-10-18 已核对仓库公开、非 Fork、未归档、默认分支为 `main`，并已设置 `herdr-plugin` topic。根清单通过 TOML 解析，版本与根 `VERSION` 一致；本机 Herdr 0.9.3 已成功以禁用状态临时链接、列出并解除链接。此验证仅覆盖清单注册，不覆盖工作台运行。市场搜索暂未命中，待清单进入默认分支并由索引刷新后复查。

当前不提供工作台安装教程。功能实现并通过验收后，根清单对应的安装来源为 `JTBlink/herdr-work`；其 ID 应保持稳定，以保留插件配置与状态目录。

`upstream/` 保留的上游清单属于来源源码。索引会枚举默认分支中的有效清单，后续导入这些目录时需核对市场是否同时列出上游插件，避免将其误认为工作台入口。

## 自动索引条件

- GitHub 仓库公开，非 Fork、未归档。
- 仓库带有 `herdr-plugin` topic。
- 默认分支含有必需元数据可解析的 `herdr-plugin.toml`；可位于根目录或子目录。
- 清单提供 `id`、`name`、`version`、`min_herdr_version`，并明确声明支持的 `platforms`。

官方约每 30 分钟刷新索引；按仓库展示卡片，并列出其中有效的插件清单。无需提交审核申请，收录也不代表官方审查或背书。

## 实现阶段需要补齐

- 与实际工作台能力一致的清单、构建入口、安装和卸载说明。
- 运行依赖、最低宿主版本、声明支持的平台及各平台实际验证结果。
- 安装、启动、退出、重启恢复、升级与异常处理的验收。
- 保留所复用源码的版权和许可声明；单独核对传递依赖。
- 对外示例、日志和截图脱敏，不提交凭据、内网地址、个人路径或本机注册表。

截图、tag、Release、徽章并非该页面列出的索引前提；它们不能代替有效清单与实际可用功能。

## 发布后验收

1. 检查已审阅的实现与清单确实存在于远端默认分支的同一 commit。
2. 核对公开状态、非 Fork、未归档及 `herdr-plugin` topic。
3. 从干净环境执行 `herdr plugin install JTBlink/herdr-work`，验证清单内实际入口。
4. 等待索引刷新，在 [Marketplace](https://herdr.dev/plugins/) 搜索 `JTBlink/herdr-work`，核对名称、版本和安装路径。

默认分支与 topic 只证明符合索引输入条件；市场实际出现卡片后才能记录“已收录”。
