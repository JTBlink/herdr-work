<!-- herdr-work-internal-modules:M07 -->
# [HerdrWork][M07] 整合文件树、Git 与 Diff 到内部 Code

## 目标与范围

将 Sidebar Tree/Git 与 Reviewr Git/Diff 的实际调用迁到 herdr-work-code，并接收 Engine 当前路径/目录实例/index/content 校验。保留 SCM 与审查读取的不同语义，通过统一快照/计划/执行接口提供行为。

## 实现位置

Sidebar tree/git、Reviewr git/diff/highlight 中相关逻辑、Engine code.rs，目标 herdr-work-code 与 TUI 着色模块。

## 验收标准

- [ ] 真实嵌套仓库/文件树/忽略规则、staged/unstaged/working/branch Diff 及二进制/重命名场景按适用 fixture 对照。
- [ ] stage/unstage/commit 只写明确 worktree；路径逃逸、目录实例替换、index/content 变化、阻塞 hooks 与未知提交结果检查仍有效。
- [ ] Code 不依赖 Review 或 TUI；着色/渲染进入 TUI，业务接口不暴露 Ratatui 类型。
- [ ] TUI Files/SCM 经过 Engine 调用新模块的实际实现，无另一套文件/Git 算法和旧来源运行副作用。

## 父规格与依赖

父规格：#17。直接阻塞：#22。

## 共同约定

复用已导入的原实现，保留许可证、版权与来源。macOS 本机验证，Linux/Windows 以同一源码 SHA 的 GitHub CI 为准；归档任务需真实内容与 SHA256 校验。ready-for-agent 仅表示描述充分，仍须先完成阻塞项。旧名称与旧插件数据兼容不在产品范围；不自动删除用户个人文件，不操作用户已有 pane 或启动真实 Agent。
