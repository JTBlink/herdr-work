# macOS 快捷键验证

日期：2026-10-31。平台：macOS Apple Silicon。

## 行为

- Explorer、搜索和 Git 支持 `Command+P` / `Command+F`，保留 Ctrl 组合键；折叠侧栏自动展开并聚焦对应输入。
- 从 Git 转到已打开的搜索时，显式搜索快捷键将焦点放到查询框。
- 终端启动时协商增强键盘协议，退出时弹出协议栈。来源参考界面和生成文档保留规范名称。

## 检查

以下为开发阶段在 v0.1.13 基线加本次快捷键改动上的检查。工作区存在其他并行修改，本次候选在独立 worktree 中构建；终端验收使用独立 target 下的两个候选程序，避免共享构建目录中的程序被其他构建替换。正式发布验收以发布提交对应的三平台 CI、归档和安装 smoke 结果为准。

- 默认 workspace 普通测试：587 通过，4 项既有忽略。首次共享 target 运行在最后的 TUI rustdoc 阶段无法查找 Engine crate；独立 target 中重跑全部 workspace doctest 通过。首次命令整体不记为通过。
- 侧栏快捷键、折叠和设置回归：3 项通过，覆盖展开/折叠、三个活动、输入框数字与提交草稿保护。
- Python unittest：41 项通过。
- Code 模块严格 Clippy、rustfmt 和 diff 空白检查通过。

复现命令：

```sh
cargo test --workspace --locked
cargo test -p herdr-work-code --features sidebar-ui --test macos_shortcuts --test sidebar_collapse --test settings_ui --locked
cargo clippy -p herdr-work-code --features sidebar-ui --all-targets --locked -- -D warnings
python3 -m unittest discover -s scripts/tests -v
```
