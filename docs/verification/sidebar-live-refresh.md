# Sidebar 外部改动自动刷新

日期：2026-10-31。平台：macOS Apple Silicon。

## 行为

默认 Sidebar 的 Explorer 每秒在后台读取当前可见目录，Git 侧栏约每 1.5 秒读取状态。Git 装饰继续使用已有后台状态与忽略缓存，取消默认失焦暂停；显式设置 `HERDR_WORK_CODE_BACKGROUND_REFRESH=0` 时保留失焦暂停的节流选项。

目录读取只覆盖当前工作目录及已展开目录，不增加全盘扫描或插件配置访问。根目录、展开集合或隐藏文件设置已经改变时，丢弃旧目录结果。按文件路径保留选择和顶部滚动锚点；自动刷新不回写其他侧栏共享的选择状态。

Git 状态按仓库路径匹配，保留各仓库草稿、光标、折叠目录及输入焦点。编辑提交说明时，即使原选中改动消失也不能把输入目标切到下一仓库。手动刷新使旧接收通道失效；菜单打开时暂缓应用 Git 列表结果，避免按旧行号执行菜单动作。Graph 的后台结果也遵循此菜单边界。

## 验证

```sh
cargo test -p herdr-work-code --features sidebar-ui --lib live_refresh --locked
cargo test -p herdr-work-code -p herdr-work-tui --locked
cargo clippy -p herdr-work-code -p herdr-work-tui --all-targets --no-deps --locked -- -D warnings
cargo fmt --check
cargo build -p herdr-work --locked
python3 scripts/tests/sidebar_refresh_fixture.py target/debug/herdr-work
git diff --check
```

5 项新增单元测试验证外部新增、重命名、删除，目录折叠后的旧结果失效，文件选择和滚动锚点，提交草稿与焦点，外部暂存，以及手动刷新丢弃旧结果。另验证多个仓库中原选中改动消失后，后续输入仍进入原仓库草稿。

真实程序 PTY fixture 使用私有宿主协议，将侧栏标记为失焦、另一工作窗格标记为聚焦；不发送刷新按键，直接从测试进程修改文件。验证 Explorer 自动显示新增和重命名，Git 自动显示修改和删除，正在输入的提交说明保留，且没有调用 pane/tab/workspace 聚焦操作。

该验证使用临时仓库和专属 profile，不操作用户会话；不将私有协议 fixture 当作真实宿主窗口人工验收。Linux/Windows 原生交互未执行。

## 本机安装验证

按用户要求执行 `sh herdr-work-plugins.sh install`，从当前源码构建 macOS Apple Silicon Release，校验归档并安装到仓库 `bin/`，通过官方 `herdr plugin link` 更新 `jtblink.herdr-work`。随后对已安装的 `bin/herdr-work` 执行相同的失焦刷新 PTY fixture，通过；`python3 scripts/smoke.py --archives dist` 的归档隔离安装与运行验证也通过。

仅对经前台进程核实为本产品的旧侧栏发送正常退出请求，再通过官方插件 `show-git` 动作打开新进程；核对新进程身份和 ready 标记。其他 Agent 窗格及独立 Projects 插件保留。
