# 仓库协作指南

## 结构与边界

HerdrWork 是 Rust 终端工作台。当前发布基线的根 workspace 包含 `crates/herdr-work-core`（契约）、`crates/herdr-work-engine`（编排与持久化）、`crates/herdr-work-tui`（唯一终端与输入）、`crates/herdr-work`（CLI），以及 `crates/herdr-work-code`（迁移的文件/Git/搜索/编辑实现）和 `crates/herdr-work-projects`（独立产品后端入口）。

Sidebar 已迁入 `crates/herdr-work-code`，生产默认库排除独立插件生命周期；Projects 的 CLI 库已合入 `crates/herdr-work-projects`，底层实现位于 `crates/herdr-work-task-{data,flow,board,web}`；Reviewr 暂留 `upstream/`。两个产品二进制统一从根构建及 target 输出，任务 worker 通过本地 Rust 依赖复用 Projects，避免原后端直接启动时使用第三方默认资料。用户已确认后续作为产品内部模块长期维护，全部生产 Rust package 迁入 `crates/`，统一 workspace、锁文件和构建；实施规范见 `docs/architecture/herdr-work-internal-modules.md`、`docs/adr/0001-first-party-workspace.md`。这替代必须保留原工程布局及仅四个 crate 的长期约束，完整九模块迁移尚未完成，不能把目标结构写成当前实现。

必须实际复用和迁移来源业务逻辑、测试与资源，保留许可证、版权和来源记录，不能用重写同名功能替代复用。采用先机械迁移、后整理模块接口的步骤，移除不再使用的原插件安装、更新及重复界面。当前补丁记录仍在 `docs/upstream/source-map.md`，任务完成后转入 `docs/provenance/`。参考副本 `reference-plugins/` 不参与构建或 Git。

新项目只支持当前 HerdrWork 名称、配置和资料目录，不读取旧环境变量，不迁移旧目录，不保留旧命令/收据/锁/标记或旧插件数据导入。普通安装所有权、失败恢复、当前名称的安装回滚及审查状态恢复仍需保留；不自动删除用户旧文件。

GitHub 仓库仍为 `JTBlink/herdr-work`，产品、自有类型与 crate 统一为 HerdrWork/herdr-work，主命令为 `herdr-work`；原项目名称保留在来源记录与许可证。独立 Sidebar、Projects、Reviewr 插件不得被本产品读取配置、接管、关闭、更新或卸载；复用限于项目内部代码。

## 构建与检查

工具链为 Rust 1.99.0，Python 3.12+。从仓库根执行：

- `cargo fmt --check`
- `cargo test --workspace --locked`
- `python3 -m unittest discover -s scripts/tests -v`
- `python3 scripts/build.py --target aarch64-apple-darwin`（在对应平台替换 target）
- `python3 scripts/package.py create --target aarch64-apple-darwin`
- `python3 scripts/smoke.py --archives dist`

Linux/Windows 编译、归档和隔离安装测试走 GitHub Actions。Windows 不包含 Unix Projects 后端，文档及产物清单必须明确能力限制。原生 UI/Agent 手动场景为建议，不冒充已经验证。

## 编写风格与命名约定

文档使用简洁中文，保持上下文风格一致。README 面向使用者，保留功能描述、项目声明与参考来源；技术方案、源码准备和重构细节写入独立文档，不在 README 展开。使用明确的 Markdown 标题、命令代码块和仓库内相对链接。文档文件名使用小写字母与连字符，例如 `plugin-merge-feasibility.md`。

保留调研文档现有的元数据。引用上游源码时固定 commit 或 tag，明确区分已核实行为、技术提案与已批准决策。Rust 使用 rustfmt；按 core、engine、tui、CLI 边界放置逻辑。

## TUI 组件与绘制规范

本项目沿用 Ratatui + Crossterm，保持单 TUI、唯一终端与输入事件循环，实际复用 Sidebar/Reviewr 业务实现。TUI 开发或审查时读取 `$rust-dev` 技能的 `references/tui-development.md`；组件选型、绘制、输入、异步结果与交互验证规则统一维护于该参考，不在仓库重复维护组件清单。实际依赖以本仓库 Cargo 清单和锁文件为准。

## 验证要求

工作台使用 Cargo 集成测试与 Python unittest 验证业务和归档边界。来源测试保留，不把未执行的测试记为通过。文档修改需核对链接、路径、事实依据及其与 README 的一致性。

后续重构需提供可复现的检查，覆盖工作上下文、任务与 Agent 关联、异步结果失效、审查发送目标、状态隔离、生命周期、升级和并发事件。分别验证 macOS 与 Linux 后再声明支持。

## 提交与合并请求

初始提交采用 `docs: <简洁中文摘要>`。文档提交沿用该格式，其他改动选择与内容对应的描述性前缀。

每个合并请求聚焦一个主题，说明目的、行为或文档变化、验证结果，并关联相关 GitHub issue。实现界面后，可见交互改动应附截图。

## 脱敏与配置管理

对外分享的示例、日志、截图和 issue 描述必须脱敏。使用明确占位符替换凭据、个人信息、内网地址和本机专有路径。`.env.example` 不得包含真实密钥。引入上游代码时保留原版权和许可证声明。

## Agent skills

### Issue tracker

规格和任务使用 GitHub Issues：JTBlink/herdr-work。详见 docs/agents/issue-tracker.md。

### Triage labels

保留默认五个 triage 标签。详见 docs/agents/triage-labels.md。

### Domain docs

采用单上下文：根 GLOSSARY.md 与按需创建的 docs/adr/。详见 docs/agents/domain.md。
