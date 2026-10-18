# Workdesk 首版验收

任务实时状态以 [GitHub Issues](https://github.com/JTBlink/herdr-work/issues) 为准。以下是实现入口与可复现标准，不将手动场景写成已完成的自动验收。

## 用户确认的关闭标准

真实代码实现并复用上游业务逻辑，编译通过即可关闭。macOS 本机构建；Linux/Windows 以固定源码 SHA 的 GitHub Actions 成功记录为依据。打包任务必须产生真实归档，通过内容和 SHA256 校验。手动交互、性能、生命周期演练为建议，不阻塞关闭。

| Issue | 实现入口 | 可复现验收 |
| --- | --- | --- |
| #2 | 四个 crates、upstream、scripts/build.py | 锁定工具链和 Cargo.lock，编译 Workdesk 与 Unix Projects 后端；doctor 报告固定来源 |
| #3 | engine/tasks.rs、host.rs、tui/state.rs | 原 tasks::list 快照，保留 attempt、terminal/socket、worktree 实例；唯一匹配才绑定；过期 epoch 不更新当前视图 |
| #5 | engine/review.rs | 评论锚定显示时的 fingerprint，原子落盘；重启、编辑、文件改变和工作目录实例替换测试 |
| #6 | engine/review.rs、workdesk/tests/host.rs | 原格式化和发送传输；固定评论版本和目标，Unknown 禁重发；聚焦失败不撤销送达 |
| #7 | engine/code.rs、tui/state.rs | 原 Sidebar Tree、Git::discover_all/status，显式 repository root；文件预览、SCM 与 Diff 联动 |
| #8 | engine/code.rs | 原 Sidebar stage/unstage，写锁和工作区实例、index、文件内容预期校验；真实 Git 测试 |
| #9 | engine/code.rs | 原 Sidebar commit，提交前 HEAD/index 再校验，持久化操作结果与显式核对 |
| #10 | engine/tasks.rs、Projects CLI/keeper 补丁 | 原 add/delegate/stop/reopen；独立 keeper；UI 退出、暂停自动调度和停止 Agent 为不同操作 |
| #11 | engine/lib.rs、review.rs、tasks.rs | 恢复执行选择/草稿，Sending 转 Unknown，不自动重试；reconcile 保存显式核对证据 |
| #12 | engine/migration.rs | 只读预览、源/复制 SHA、备份、持久化导入 intent、中断恢复及拒绝覆盖新数据的回滚测试 |
| #13 | scripts/package.py、install.py、smoke.py | macOS 真实归档、许可、manifest、SHA、安装运行、升级回滚与卸载后保留数据 |
| #14 | .github/workflows/ci.yml、build.yml、release.yml | 三平台原生编译/测试/打包/安装；全部成功后 draft 上传附件，最后发布 |

## 本机已验证命令

```sh
cargo fmt --all --check
cargo test --workspace --locked
python3 -m unittest discover -s scripts/tests -v
python3 scripts/build.py --target aarch64-apple-darwin
python3 scripts/package.py create --target aarch64-apple-darwin
python3 scripts/smoke.py --archives dist
```

工作流额外通过 actionlint。原始上游源码的空白与 Markdown 硬换行保留；空白检查针对自有代码、文档与必要补丁。上游完整测试及 fixture 保留，不将未执行的上游全量测试记为通过。

## 范围与尚需实际会话验证的事项

- Windows 不包含 Unix Projects 后端，支持原生文件、Git、Diff 与草稿；原生任务执行不在本次 Windows 包能力内。
- 私有宿主 fixture 验证真实 CLI 与原 socket/readiness 链路，不等同于真人 Agent 已阅读或执行反馈。宿主缺少原子 send-if-ready，因此最终检查与宿主写入仍存在宿主侧竞态。
- 本机无 Apple 公证凭据，安装时 ad hoc 签名。未声明 Developer ID 签名或 SmartScreen 信任。
- 实际 Herdr 会话中的 pane 移动、Agents 长时间运行和性能演练仍作为建议验证场景。
