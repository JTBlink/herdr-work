# Sidebar 代码迁移与产品隔离

日期：2026-10-19。导入基线：[Sidebar 081ed133](https://github.com/alexarthurs/herdr-sidebar/tree/081ed133f3faa377438b51ce435fe8a87c439593)，原 package 版本 0.15.1。

## 所有权与来源

原 `upstream/herdr-sidebar/plugins/herdr-sidebar` 已迁入 `crates/herdr-work-code`，包、库和内部命名使用 HerdrWork，自根 workspace、Cargo.lock、target 构建；原独立锁文件退出生产构建。原 MIT 许可证保留于该 crate 的 LICENSE，原说明、维护笔记与插件清单归档在本目录。移除原 fetch-or-build、redeploy 及来源仓库安装/发布脚本，防止手动执行时误下载或关闭独立 Sidebar 窗格；不构成本产品安装入口。


完整功能差异见 [Sidebar 能力对齐盘点](sidebar-capability-audit.md)。下表记录本版实际接入，不代表来源全能力已经对齐。

## 实际代码复用

| 能力 | 原实现与迁移方式 | HerdrWork 接入 |
| --- | --- | --- |
| 文件树与图标 | 原 Tree、图标分类、Git::discover_all/status、调色板 | Explorer、图标活动栏、Git 装饰、焦点与预览 |
| 文件菜单 | 原 actions::menu_entries、名称校验和 create/rename/delete、剪贴板、系统打开/显示 | 工作目录边界、操作前快照及删除确认；后台执行 |
| Quick Open / 内容搜索 | 从原 explorer_app 原位抽出 search 模块，原排序、模糊匹配、ignore walker、正则、全词及 glob 规则 | 搜索页、Ctrl+P、大小写/全词/正则和包含/排除过滤；请求序号拒绝旧结果 |
| 八类 Git 面板 | 从原 scm_app 原位抽出 scm 模型及查询分派；原 Git graph/commits/history/branches/worktrees/remotes/stashes/tags | 可展开列表与独立内容区，真实对象详情 |
| 对象菜单 | 原菜单、引用解析及 Git 参数构造迁入同一 scm 模块，来源界面也调用该模块 | 查看/复制/显示及 checkout、merge、cherry-pick、revert、reset、stash、fetch、删除等；确认后重新检查身份/HEAD/index/内容/引用，记录未知结果 |
| 暂存、取消暂存和提交 | 原 Git::stage_under/unstage/unstage_under/unstage_all/commit | Explorer 文件/目录、SCM 文件/目录/整仓入口；整仓暂存使用原 stage_under 的嵌套仓库边界；保留目录与仓库实例、HEAD、index、完整相关内容、锁与确认快照检查 |
| SCM 目录树与提交输入 | 原 scm_app 的树构建与字符/光标编辑原位移至 scm_tree/scm_input，来源界面调用共享实现 | SCM t 切换树/列表、分仓库分组、常驻提交输入；各仓库草稿与字符光标只保存于 HerdrWork git-drafts；失败保留输入 |
| 分支、同步、丢弃与 Stash | 原 BranchPicker、Git 分支方法、sync、discard 与 stash push --include-untracked | 活动仓库明确显示 branch/ahead/behind/upstream；未合并删除生成独立第二次 FORCE DELETE 确认；Unknown 通过与 CLI 共用的 reconcile/resolve 接口显式记录证据，不自动重试 |
| 提交消息建议 | 原 suggest::generate 与 diff_for_message | 后台生成；缺少 claude 或失败沿用来源文件名回退；用户已修改的草稿不被异步建议覆盖 |
| 内置文本编辑 | 直接使用原 Editor；原缓冲、Unicode 光标、选择、粘贴、查找、换行/BOM 保存和外部修改检查 | e 编辑、Ctrl+S 保存、未保存提示；目录与路径校验；二进制、无效 UTF-8、超过 1 MiB/5000 行保持只读 |
| 撤销/重做 | 在共享 Editor 内补充有界历史；64 步且历史文本最多 16 MiB，键盘及粘贴共同使用 | Ctrl+Z、Ctrl+Y/Ctrl+Shift+Z；保存后撤销仍正确标识 dirty |
| 外观设置 | 原宽度步进、颜色主题、图标主题和样式模型抽至无插件 I/O 的 appearance | 资料目录内 ui.json 原子保存：宽度、停靠、颜色/图标、隐藏文件、Git 装饰及快捷键 |

Git 确认指纹包含相关 tracked/untracked 内容，预算为 256 MiB。超预算或内容无法核对时，Explorer/SCM 仍返回路径和状态，并通过 `write_error` 明确标识只读安全快照；该快照不能授权暂存，修改计划仍必须取得完整指纹。

HerdrWork 保持唯一终端和事件循环。源插件的自动拆分窗格、接管/更新、旧配置读取、宿主 viewer 生命周期等模块通过 `reference-ui` 特性隔离，**生产构建不启用此特性**。该特性只供保留来源界面的显式测试；不是独立 Sidebar 的安装或升级入口。源码内不保留第二套搜索或 Git 算法。

## 专属任务 worker

`crates/herdr-work-projects` 为独立产品入口，调用已导入的 Projects 库。入口在启动线程前强制设置自己的 home/data/settings/reports，资料由 HERDR_WORK_HOME 或默认 ~/.herdr-work 决定；不以继承的 HERDR_PROJECTS_* 路径决定资料，不依赖调用方正确设置 managed 标记。继承的第三方插件 ID/context 会清除，原 Agent 回调中的空插件身份保留。主程序、hook 和生成 launcher 显式传递 HerdrWork home，防止回到独立插件目录。


## 验证边界

- 根 workspace 测试包含 Code 336 项原库测试、编辑器/搜索/面板/菜单/设置交互、文件与 Git 状态保护和专属 worker 的两种资料路径隔离；Code 原库中的 3 项测试沿用原有 ignored 标记。
- 保留来源界面通过 `cargo test -p herdr-work-code --features reference-ui --bin herdr-work-code-browser --locked` 单独验证；不能把仅核心库通过等同于原界面测试通过。
- macOS 私有 PTY 实际验证菜单/设置、原编辑器选择/粘贴/保存、预览、真实 stage/unstage/Diff、创建任务、80 列和终端恢复。Windows 运行 Rust 渲染和业务检查，不声称验证过原生 Unix PTY 或 Unix 任务执行。
- 另外运行过原 Projects 的独立 fixture 基线：828 通过、6 失败。失败涉及两项旧目录迁移时序、旧共享 Cargo 配置链接、原生只读规划沙箱前提、旧插件进程接管和旧 warm-start 构建路径。这不是本次新增生产功能的通过证据，不修改或忽略后冒充通过；保留给 #20 的来源测试迁移分析。原始日志仅存本地，未提交。

界面图为临时 Git 仓库与测试数据渲染的 fixture，不包含用户工作目录或 Agent 会话：

![Explorer fixture](../verification/ui-explorer.png)

![Source Control fixture](../verification/ui-source-control.png)
