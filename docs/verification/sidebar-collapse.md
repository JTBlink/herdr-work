# 侧栏折叠与关闭

日期：2026-10-29。平台：macOS Apple Silicon。

## 行为

- 默认侧栏的 `«` 将正文折叠为一列图标，保留文件、搜索、Git、任务和设置入口。
- `»`、Enter 或右方向键展开原视图，保留文件树展开状态、选择和提交草稿；文件、搜索、Git 图标展开并选择对应视图。任务入口保持图标列并独立打开看板，设置入口展开设置。
- 展开时右上角、折叠时底部的 `×` 关闭整个侧栏。宿主内继续使用来源草稿保存、hide 和 snooze；无宿主时通知唯一终端循环退出。`b` 保留隐藏语义。
- 折叠目标固定为图标宽度 × 1.2 向上取整，即 3 个终端列，实际宽度遵循 Herdr 最小分栏限制。本机 160 列私有宿主中的窗格外框最小宽度为 13 列，活动图标在实际宽度内居中，不显示悬停文字。直接调用 `layout.set_split_ratio` 设置 0.04 或 0.025，宿主仍返回 0.1；当前 API 无法兑现 3 列的实际窗格宽度，需宿主支持。
- 折叠期间继续心跳和后台刷新，不将窄窗口尺寸交给来源 App 的首选宽度修复。折叠状态仅属于当前组件实例，不改写保存的正常侧栏宽度。

![侧栏展开与折叠](ui-sidebar-collapse.png)

上图由实际 Sidebar 组件的 Ratatui TestBackend 帧导出，使用临时仓库与资料，展示 38 列展开和 3 列折叠布局；不是宿主窗口截图。

## 验证

```sh
HERDR_WORK_TEST_FRAMES=<绝对输出目录> cargo test -p herdr-work-code --features sidebar-ui --test sidebar_collapse --test settings_ui --locked
cargo test --workspace --locked
python3 -m unittest discover -s scripts/tests -v
cargo build -p herdr-work --locked
python3 scripts/docking_smoke.py --binary target/debug/herdr-work --output <绝对输出目录>
cargo fmt --check
git diff --check
```

组件回归覆盖 Emoji/Material、左右停靠、Explorer/Search/Git、图标点击、任务请求、展开设置、原视图保持，以及提交输入获焦时折叠与草稿保留。关闭测试覆盖无宿主终端退出；私有 Herdr 测试实际点击 `«`、`»` 和 `×`，验证宽度恢复、折叠跨布局恢复定时器保持、关闭成功与原工作分栏树恢复。

私有宿主 8 组场景通过：左右停靠、上下分栏、嵌套分栏、已有侧栏，以及显式激活和静默 ensure。Python unittest 41 项通过。本机开发构建、格式及 diff 空白检查通过。测试不连接用户宿主会话。

未执行 Linux/Windows 原生交互或人工操作验收。

本次 `0.1.10` 根 workspace 回归：568 项通过、4 项按既有配置忽略；Python unittest 41 项通过。

已增加[宿主精确比例补丁](../patches/herdr/README.md)，并完成测试版宿主 8 组实际 3 列验证及私有会话在线交接后重连验证。当前用户会话仍需单独切换宿主，普通侧栏重装不会改变旧宿主限制。
