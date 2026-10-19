<!-- herdr-work-internal-modules:M02 -->
# [HerdrWork][M02] 默认工作区采用 Explorer 与 Source Control 布局

## 目标与范围

对照 Sidebar 的活动栏、文件/SCM 视图和状态颜色，默认显示 Explorer；任务页通过活动栏或快捷键进入。统一活动栏、可切换侧栏、预览/Diff 内容区与状态栏；复用 Tree/Git 和 Sidebar 样式。

## 实现位置

crates/herdr-work-tui/src/{state,actions,view,layout,sidebar_view,mouse}.rs、scripts/ui_smoke.py。

## 验收标准

- [ ] 真实默认启动在任务为空或任务后端失败时仍显示文件树和预览，不再强制绘制 Tasks 左栏。
- [ ] 键盘与鼠标切换 Files/Git/Tasks/Review；目录展开、文件预览、Git 状态标记、Staged/Changes 和真实 Diff 可用。
- [ ] 在临时 Git 仓库实际暂存/取消暂存并核对 index；创建任务、Tab 焦点、预览滚动和晚到预览保护通过。
- [ ] 80 列可见侧栏和内容；更窄时按焦点显示；退出恢复终端并关闭鼠标捕获。
- [ ] 私有 PTY 检查分别等待 Tasks 与 Files/Git 就绪，再走实际操作链，不把布局框架出现当作数据加载完成。

## 父规格与依赖

父规格：#17。直接阻塞：无。

## 共同约定

复用已导入的原实现，保留许可证、版权与来源。macOS 本机验证，Linux/Windows 以同一源码 SHA 的 GitHub CI 为准；归档任务需真实内容与 SHA256 校验。ready-for-agent 仅表示描述充分，仍须先完成阻塞项。旧名称与旧插件数据兼容不在产品范围；不自动删除用户个人文件，不操作用户已有 pane 或启动真实 Agent。
