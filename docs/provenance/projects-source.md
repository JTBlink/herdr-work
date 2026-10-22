# Projects 源码迁移

日期：2026-10-22。来源基线：[Projects 92a2813b](https://github.com/eliasstravik/herdr-projects/tree/92a2813be9a7c2f8cda737f6b78ed21e8413c3ce)，原版本 1.2.2。关联既有整合规格 [#17](https://github.com/JTBlink/herdr-work/issues/17)，本记录不修改云端任务状态。

## 目录与构建

| 原路径 | 当前权威位置 |
| --- | --- |
| `upstream/herdr-projects/src` | `crates/herdr-work-projects/src`，原 CLI 库合入现有 worker package，库名 `herdr_work_tasks` |
| `crates/core` | `crates/herdr-work-task-data` |
| `crates/flow` | `crates/herdr-work-task-flow` |
| `crates/board` | `crates/herdr-work-task-board` |
| `crates/web` | `crates/herdr-work-task-web`，包含 Web 页面 JS/CSS |
| `tests` | `crates/herdr-work-projects/tests/source`，保留原单一测试程序与 frames/endings fixture |
| `scripts`、`bin`、`.cargo` | `crates/herdr-work-projects/tests/fixtures/projects`，仅供来源测试复制到临时目录执行 |
| 原文档、说明、插件清单、素材、维护规则 | [projects](projects/)，历史 Cargo 清单保存为 `Cargo.toml.txt`，原发布 workflow 存于 `github/` |

实际移动已有实现，保留任务存储、Attempt/Run、委派、keeper、回调、锁/心跳、看板和 Web 业务；没有另写任务状态机。每个迁入 crate 保留原 MIT LICENSE。包名改为 HerdrWork，版本继承根版本；内部 `sw_core`/`sw_flow` 等依赖别名用于减少业务改动，不是旧用户命令。

原 workspace、独立 Cargo.lock、生产启动脚本及独立二进制入口退出构建。十个根成员共同使用根锁和 target；保留 `serde_json` 的 `preserve_order`、`float_roundtrip`，沿用原 toml 0.9 与其他依赖版本。Rust 源码统一经过 rustfmt。来源 docs 工具仅在 `reference-ui` 特性中保留，用路径映射读取迁入的唯一源码副本。

## 入口与平台边界

发行仍只有 `herdr-work` 与 `herdr-work-projects`。worker 资料隔离及 HerdrWork 托管限制沿用现有实现，不读取、接管、关闭或更新独立 Projects 插件。旧安装/接管实现仍供来源场景核对，产品入口在启动业务前固定托管身份；整合不启用来源的安装、更新和旧数据迁移。

`herdr-work-projects-reference` 仅在显式 `reference-ui` 特性下构建，供临时目录中的来源 CLI 场景使用，不进入发行归档。来源脚本、Cargo 配置和 workflow 不参与产品构建。原 300 行压缩源码布局检查不适用于迁入后统一 rustfmt 的布局，单项明确忽略；来源 map、按键与资源检查继续运行。

Projects 四个底层成员及 worker 业务只支持 Unix。Windows CI 明确排除四个底层成员，主产品和 worker 的不可用错误沿用原契约；不宣称原生 Windows 任务执行可用。

这是 Projects 的工程迁移。task-board/task-web 仍是过渡成员，前台 Engine 仍通过 worker CLI 读写任务；后续 Tasks 内聚、Runtime 分离、单 TUI 职责整理和 Reviewr 迁移依照[内部模块架构](../architecture/herdr-work-internal-modules.md)继续，最终九模块尚未完成。

## 可复现检查

```bash
cargo fmt --check
cargo test --workspace --locked
cargo test -p herdr-work-projects --features reference-ui --test source --locked
python3 -m unittest discover -s scripts/tests -v
python3 scripts/build.py --target aarch64-apple-darwin
python3 scripts/package.py create --target aarch64-apple-darwin
python3 scripts/smoke.py --archives dist
```

迁移前原来源套件实际执行：828 通过、6 失败（834 项）。已有失败为两项旧目录迁移锁/时序、原 Cargo 配置 symlink 假定、原生只读规划沙箱前提、旧插件进程接管、旧 warm-start 构建路径。本次保留这些场景及失败证据，不把原来失败的场景宣称为通过。

迁入后五个 Projects package 的默认测试、根 `cargo check`、根 rustfmt、28 项 Python 测试、macOS release 构建和归档校验通过。原 map 测试的相对路径已适配，旧 linker 夹具按实际导入的普通文件校验与 linker.sh 字节一致，避免要求导入归档中不存在的 symlink。迁入后的 834 项以独立进程分组执行，最终逐项结果为 **828 通过、5 失败、1 忽略**。doctor 夹具路径修正后单项通过；远端看板等待超时后补跑通过，失败诊断补充机器状态。5 项失败均与迁移前基线一致：

- `home_folder_more::real_programs_a_lock_file_that_was_replaced_between_opening_it_and_locking_it_is_not_taken_for_the_lock`
- `home_folder_more::real_programs_the_old_command_runs_at_every_moment_of_a_real_move_and_a_lock_held_through_it_stays_held`
- `plan_steps::without_a_read_only_mount_no_plan_is_written_in_a_repo_and_a_plan_said_not_saved_is_the_new_plan`
- `take_over_more::real_programs_herdr_superworks_own_processes_are_stopped_and_no_other`
- `warm_start::real_programs_what_was_built_with_the_other_folders_path_in_it_is_built_again`

唯一忽略项为原独立仓库的 300 行压缩源码布局检查；未通过的五项没有被改成忽略或通过。

完整 workspace 首次检查遇到当前工作区的其他问题：宿主激活 fixture 缺 tab，TUI 异步 fixture 等待超时，两个 doctest 缺少共享 target 中的 Code rlib。随后单独复核的 TUI 81 项与两个 doctest 已通过。完整 workspace 串行复核仍在 host 目标失败：`ambiguous_post_send_receipt_is_unknown_and_cannot_resend` 的发送次数断言、`explicit_initial_activity_and_root_win_over_an_unrelated_saved_binding_in_private_pty` 的 PTY 画面断言；该次检查并未全部通过。隔离安装 smoke 已进入 UI 检查，但原提交区符号断言与当前压缩布局不符；归档生成和校验本身通过。这些不记为通过，也未为此次源码移动更改无关 UI/停靠实现。

已核对 771 个来源输入均有迁入位置（独立锁除外），随后删除整个 `upstream/herdr-projects`，包括旧本地 target；仓库不留第二份 Projects 生产源码。跨平台原生执行与发布未在本机完成，不以 macOS 结果替代。
