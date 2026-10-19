# [HerdrWork][03] 在所选执行中阅读真实 Diff

GitHub: [[HerdrWork][03] 在所选执行中阅读真实 Diff](https://github.com/JTBlink/herdr-work/issues/4)

命名及验收约定同步快照；实时状态以 GitHub 为准。

## 当前关闭标准（用户确认）

- 对应任务必须有真实实现，复用指定上游业务逻辑，不以空实现或仅改文档替代。
- 对应实现编译通过即可关闭；记录编译命令和结果。macOS 可本机验证；Linux/Windows 使用 GitHub Actions 成功记录（run URL 与源码 SHA）。
- 涉及打包的任务还需生成实际归档并通过内容和 SHA256 校验。只写工作流而未成功构建不视为编译完成。
- 原手动交互、安装、性能和生命周期验证保留为建议，不作为关闭阻塞；任务关闭不等同于所有运行场景已人工验收。
- 产品名称为 HerdrWork，主二进制为 herdr-work（Windows 为 herdr-work.exe）；四个自有 crate 为 herdr-work-core、herdr-work-engine、herdr-work-tui、herdr-work。GitHub 仓库仍为 JTBlink/herdr-work，上游保留原名和结构。

## Parent

#1 — https://github.com/JTBlink/herdr-work/issues/1

**What to build:** 用户选定执行后，直接阅读其 worktree 的改动，切换文件、审查范围和比较基线时得到相应 Diff。

**Blocked by:** #3（切片 02）

## 业务源码复用


实现时列出仍实际调用的来源逻辑、必要适配和保留的上游测试；不能以重新实现同名功能代替复用。

## 实现范围

- 文件、旧/新侧、基线和范围可见；Diff 属于显式 worktree，目录丢失或同路径重建时重新发现。
- 同上下文的旧 generation 与跨任务的旧 epoch 均不落入当前阅读区。
- Git 读取在后台且可取消/超时；读取失败保留上下文并给出可恢复反馈。

## 建议验证（不阻塞关闭）

复用 Diff/model/world 测试；两个 worktree、rename、staged/unstaged、窄屏、慢读取和乱序完成演示。

以使用者可见行为验证，不仅测试动作转发。未接入的原菜单/副作用保持关闭，不能因复用代码而默认启用。

## 背景

本任务属于复用优先工作台首版；采用四个自有 crate 的工作台 workspace、保留原结构的 upstream、单 TUI、Projects 原后端体系及隔离数据。执行前读取父规格中完整的首版规格、架构、领域术语及测试约定。阻塞关系同时使用 GitHub 原生依赖，全部阻塞项关闭后才能开始。
