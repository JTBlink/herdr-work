# macOS 快捷键验证

日期：2026-10-31。平台：macOS Apple Silicon。

## 行为

- HerdrWork 托管看板的 Alt 提示改为 Option，菜单标明 `Option (⌥)`；设置接受 `Option` / `Control` 写法，保存与终端传输仍使用规范和弦。
- Explorer、搜索和 Git 支持 `Command+P` / `Command+F`，保留 Ctrl 组合键；折叠侧栏自动展开并聚焦对应输入。
- 侧栏列表按 `4` 打开主看板；搜索字段、Quick Open 和提交草稿中的 `4` 保留为文本。
- 从 Git 转到已打开的搜索时，显式搜索快捷键将焦点放到查询框。
- 终端启动时协商增强键盘协议，退出时弹出协议栈。来源参考界面和生成文档保留规范名称。

## 检查

以下为开发阶段在 v0.1.13 基线加本次快捷键改动上的检查。工作区存在其他并行修改，本次候选在独立 worktree 中构建；终端验收使用独立 target 下的两个候选程序，避免共享构建目录中的程序被其他构建替换。正式发布验收以发布提交对应的三平台 CI、归档和安装 smoke 结果为准。

- 默认 workspace 普通测试：587 通过，4 项既有忽略。首次共享 target 运行在最后的 TUI rustdoc 阶段无法查找 Engine crate；独立 target 中重跑全部 workspace doctest 通过。首次命令整体不记为通过。
- 侧栏快捷键、折叠和设置回归：3 项通过，覆盖展开/折叠、三个活动、输入框数字与提交草稿保护。
- Python unittest：41 项通过。
- 真实 PTY：侧栏点击与数字快捷键、CSI-u `Command+F` 后输入数字、重复激活和失败重试通过；看板 Option 菜单、首屏、粘贴输入、关闭及草稿保留通过。
- Code 模块严格 Clippy、rustfmt 和 diff 空白检查通过。
- 扩展来源看板测试：240 通过、16 失败。失败涉及既有 `✕` 关闭按钮导致的标题、宽度和旧帧断言不一致，未在本次快捷键修改中改写这些快照。
- 对 task-data/flow/board 一并执行严格 Clippy 时，task-data 中 50 项未修改代码的告警阻止检查，未记为通过。

复现命令：

```sh
cargo test --workspace --locked
cargo test -p herdr-work-code --features sidebar-ui --test macos_shortcuts --test sidebar_collapse --test settings_ui --locked
cargo test -p herdr-work-projects --features reference-ui --test source board_ --locked -- --test-threads=1
cargo clippy -p herdr-work-code --features sidebar-ui --all-targets --locked -- -D warnings
python3 -m unittest discover -s scripts/tests -v
python3 scripts/tests/projects_entry_fixture.py <候选目录>/herdr-work
python3 scripts/tests/board_runtime_fixture.py <候选目录>/herdr-work-projects
```

PTY 验收使用独立资料目录，不读取或修改用户看板。实际 Mac 键盘及外层终端对 Command/Option 的截获未作手动验收；Linux/Windows 原生交互未在本机验证。候选程序没有替换用户正在使用的安装。
