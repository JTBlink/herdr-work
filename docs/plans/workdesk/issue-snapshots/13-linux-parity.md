# [Workdesk][13] Linux/Windows GitHub Actions 编译、打包与 Release 流程

GitHub: [[Workdesk][13] Linux/Windows GitHub Actions 编译、打包与 Release 流程](https://github.com/JTBlink/herdr-work/issues/14)

命名及验收约定同步快照；实时状态以 GitHub 为准。

## 当前关闭标准（用户确认）

- 对应任务必须有真实实现，复用指定上游业务逻辑，不以空实现或仅改文档替代。
- 对应实现编译通过即可关闭；记录编译命令和结果。macOS 可本机验证；Linux/Windows 使用 GitHub Actions 成功记录（run URL 与源码 SHA）。
- 涉及打包的任务还需生成实际归档并通过内容和 SHA256 校验。只写工作流而未成功构建不视为编译完成。
- 原手动交互、安装、性能和生命周期验证保留为建议，不作为关闭阻塞；任务关闭不等同于所有运行场景已人工验收。
- 产品名称为 Workdesk，主二进制为 herdr-work（Windows 为 herdr-work.exe）；四个自有 crate 为 workdesk-core、workdesk-engine、workdesk-tui、workdesk。GitHub 仓库仍为 JTBlink/herdr-work，上游保留原名和结构。

## Parent

#1 — https://github.com/JTBlink/herdr-work/issues/1

**What to build:** 参考固定版本 Reviewr 的成熟发布链路，为 Workdesk 配置 GitHub Actions 原生 Linux/Windows 构建、归档打包、校验和与版本 tag Release。使用同一源码 SHA；平台失败不发布不完整 Release。

**Blocked by:** #2（复用构建基线）。CI 配置可先编写，成功构建需基线源码接入，不等待 macOS 手动安装验收。

## 业务源码复用

以 Reviewr 0.46.0 / 859dd5b11a5280fe282de56b52378b20a54fa10a 的 CI/release.yml 为参考，采用固定工具链、平台矩阵、归档校验、草稿 Release 和全部成功后公开的模式。Workdesk 四个 crate 的代码和上游业务逻辑必须被实际编译。

## 实现范围

- 提供 PR/main/手动 CI 及 v* tag 发布；手动构建只生成候选产物，不自动公开 Release。
- Linux 与 Windows 使用原生 runner，锁定工具链与依赖；macOS 构建进入同一产物约定。
- 明确各平台实际编译/打包目标。Projects 原后端存在 Unix 依赖；Windows 必须如实标出能力范围，不将跳过后端称为完整 Windows 后端通过。
- 输出版本、目标平台、源 SHA、原版权与来源、实际可执行文件及 SHA256；缺少应有文件立即失败。
- 构建 job 只读权限，发布 job 按需 contents:write；所有平台产物来自同一 SHA；tag/version 不一致拒绝发布。
- 创建草稿 Release、上传全部校验过的产物，再公开发布；保留失败草稿供排查，不覆盖已有正式 Release。

## 关闭证据

- 真实源码的 Linux/Windows CI 编译成功 run URL 和 SHA。
- 归档与 SHA256 校验结果；如未发正式版本，候选打包成功即可，不要求为关闭任务额外创建 Release。

## 建议验证（不阻塞关闭）

原生交互、watcher、进程树、安装/升级/卸载和跨平台运行验证后续可补充，不作为本任务关闭门槛。
