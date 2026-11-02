# README 图片记录

采集日期：2026-11-02。程序：HerdrWork 0.1.20，源码提交 `5875342ea400eebfa6796db141feafba6a314fbf`。平台：macOS Apple Silicon。

## 来源与脱敏

使用独立 Herdr 0.9.5 补丁宿主、PTY 和临时演示仓库运行正式程序，采集文件、Git 和折叠状态的真实 `pane.read` 文本。未连接或截取用户工作会话。

演示仓库仅含 `src/main.rs`、`README.md` 和 `notes.md`，Git 身份使用演示占位符。快照仅保存侧栏内容，不包含宿主配置、个人路径、凭据、用户对话或终端提示符。PNG 不包含元数据。

- [文件、Git 与折叠原始快照](assets/readme-ui-020.json)
- [文件与 Git 图片](assets/workbench.png)
- [折叠与展开图片](assets/sidebar-width-095.png)

图片按等宽字符网格渲染，保留快照中的图标位置与按钮间隔，省略中间连续空白；采用统一配色，不代表终端原始主题。它们是终端文本快照图，不是桌面像素截图。

## 检查与重新渲染

本次私有宿主场景验证了完整高度停靠、三列内容加双边框、窗口变宽后的固定宽度、图标居中、展开恢复，以及关闭后工作分栏和进程保留。

在 macOS 安装 Pillow 并准备 Nerd Font Mono 字体后，可从已脱敏快照重新渲染：

```sh
python3 scripts/render_readme_images.py --font /path/to/NerdFontMono-Regular.ttf
git diff --check
```

渲染器仅读取仓库内固定快照，检查敏感文本并生成不含元数据的两张 PNG。更新程序后应重新采集快照，再渲染；不能只改图片上的版本标签。
