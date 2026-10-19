<!-- herdr-work-internal-modules:M01 -->
# [HerdrWork][M01] 清理旧项目兼容，只保留当前名称

## 目标与范围

检查 profile、CLI、任务后端、安装器和包校验中的旧名称分支。移除 WORKDESK_HOME/WORKDESK_MANAGED、旧目录迁移、旧锁/标记、旧命令/收据与旧插件数据 import/rollback-import；保留当前名称的所有权、安装失败恢复与回滚。旧个人文件不自动删除。

## 实现位置

crates/herdr-work-engine/src/{profile,lib,code}.rs、CLI、scripts/install.py/package.py/smoke.py 和来源 Projects 必要适配。

## 验收标准

- [ ] 新旧环境变量同时存在时仅当前变量生效；只有旧变量时使用 ~/.herdr-work；旧目录内容不被读取或复制。
- [ ] 旧托管变量、旧暂停文件和旧后端命令不再产生行为；当前目录和 launcher/report 隔离仍通过。
- [ ] 当前安装只接受 HerdrWork 产品及当前文件名；旧收据不被接管，同名无主文件拒绝覆盖。
- [ ] 旧导入命令与帮助/引导已移除；当前资料独占锁、Git 安全检查、审查恢复和普通安装回滚仍可用。

## 父规格与依赖

父规格：#17。直接阻塞：无。

## 共同约定

复用已导入的原实现，保留许可证、版权与来源。macOS 本机验证，Linux/Windows 以同一源码 SHA 的 GitHub CI 为准；归档任务需真实内容与 SHA256 校验。ready-for-agent 仅表示描述充分，仍须先完成阻塞项。旧名称与旧插件数据兼容不在产品范围；不自动删除用户个人文件，不操作用户已有 pane 或启动真实 Agent。
