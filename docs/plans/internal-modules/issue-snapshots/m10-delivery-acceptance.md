<!-- herdr-work-internal-modules:M10 -->
# [HerdrWork][M10] 验收统一工程的三平台构建、归档与安装

## 目标与范围

在同一源码 SHA 的干净 checkout 验收统一 workspace、两个当前产品入口和来源许可。适用平台各自真实构建、测试、打包、隔离安装并下载检查 CI 归档；正式 Release 另按用户发版要求执行。

## 实现位置

scripts/build.py/package.py/install.py/smoke.py/ui_smoke.py、根 Cargo、CI；打 tag/发布安装包不由规划自动触发。

## 验收标准

- [ ] 根 workspace/锁/target 与实际脚本一致；生产编译不引用 upstream/ 或第二个 manifest。
- [ ] macOS/Linux 完成任务回调、keeper、Git、审查及私有 PTY 操作；Windows 完成原生文件/Git/审查和明确不可用的任务入口检查。
- [ ] 归档的产品/版本/固定源码 SHA、文件集、许可和 SHA256 均校验；仅当前名称的首次安装、失败恢复、重装、回滚与卸载不损坏数据。
- [ ] 默认 Explorer、鼠标导航、真实 stage/unstage/Diff/创建任务、80 列及终端恢复实际运行；分别等待独立加载结果。
- [ ] 任务证据包含固定 commit、三平台 CI URL、实际检查命令及原生手动范围；没有执行的来源测试和会话场景不记为通过。

## 父规格与依赖

父规格：#17。直接阻塞：#26。

## 共同约定

复用已导入的原实现，保留许可证、版权与来源。macOS 本机验证，Linux/Windows 以同一源码 SHA 的 GitHub CI 为准；归档任务需真实内容与 SHA256 校验。ready-for-agent 仅表示描述充分，仍须先完成阻塞项。旧名称与旧插件数据兼容不在产品范围；不自动删除用户个人文件，不操作用户已有 pane 或启动真实 Agent。
