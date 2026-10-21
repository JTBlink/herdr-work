# Sidebar 共存验证

## 目标与边界

验证 HerdrWork 与独立 Sidebar 同时启用时，文件预览占满可用工作区域，双方文档、控制文件、进程及恢复操作保持隔离。Sidebar 基线为 v0.15.1，来源提交 `081ed133f3faa377438b51ce435fe8a87c439593`。

测试只读指定的 Sidebar 可执行文件，复制到临时目录，在新的 XDG 配置和命名 Herdr 会话内注册两个测试插件。双方保留各自自动开栏事件，使用各自临时资料；已安装 Sidebar 的配置、安装和运行实例不参与测试。测试结束核对源程序 SHA256 不变，并只停止测试自己创建的客户端和服务端。

## 运行

在 macOS 或 Linux 上准备 Herdr 和已构建的两个程序，选择一个全新输出目录：

```sh
python3 scripts/sidebar_coexistence_smoke.py \
  --binary target/aarch64-apple-darwin/release/herdr-work \
  --sidebar-binary /path/to/herdr-sidebar \
  --output .scratch/sidebar-coexistence
```

可使用 `--side left` 或 `--side right` 只运行一侧。Linux 替换 HerdrWork 的 target 路径。测试会启动私有 PTY 客户端，不切换用户当前窗口。

## 断言

- 独立 Sidebar 分别位于左右两侧，HerdrWork 接管工作子树后，三个工作进程继续运行；预览占用实际空间，独立 Sidebar 的矩形和进程保持不变。
- 预览及停车标签页事件不会生成额外 HerdrWork 侧栏。
- 独立 Sidebar 打开、关闭自己的文件预览后，HerdrWork 的文档、控制文件身份和接管布局保持不变；反向操作也不关闭独立 Sidebar。
- 连续选择文件复用同一 HerdrWork viewer；编辑取消保留文本，保存后返回预览；正常关闭和宿主直接关闭 viewer 均精确恢复原窗格、分栏方向及比例。

普通工作标签页上，两个独立插件各自自动打开侧栏仍由各自设置决定；本测试不对第三方插件做互斥、接管或关闭。

## 本次发现

2026-10-21 的真实双向预览验证复现了通用标题冲突：独立 Sidebar 将 `文件名 · preview` / `文件名 · editor` 识别为自身重启残留，可能清理仍在运行的 HerdrWork viewer。HerdrWork 改用 `HerdrWork · 文件名 [preview]` / `[editor]` / `[pinned]` 标题，创建时的占位标题也采用相同命名空间；原 Sidebar 无需修改。

本机验证平台为 macOS Apple Silicon；Linux 和 Windows 的原生共存交互不能由本机结果替代。

修正后，真实双插件在左右两种停靠位置均通过上述断言，包括双方自动事件、独立 Sidebar 的预览打开/关闭、HerdrWork 文档与控制文件不变、正常与异常关闭后的精确恢复，以及 Sidebar 源程序 SHA256 不变。
