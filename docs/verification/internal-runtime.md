# 内部运行设施与历史清理验证

日期：2026-10-31。平台：macOS Apple Silicon。

## 当前代码

生产 Rust 包均在 `crates/` 下；取消外部审查组件依赖、目录、打包路径和资料引用。保留现有文件、Git、任务和产品自身的审查会话。

Git 身份和索引校验通过 Code 客户端读取；差异行使用现有文本比较算法；显式宿主请求复用 Code 的有界 IPC。进程执行器保留进程组 / JobObject、截止时间及管道排空行为，机械迁入 Engine，并保留 MIT 版权与发行许可证。

## 已执行检查

- 独立工作区全仓：588 项通过、4 项既有忽略；新增 IPC / Git 校验用例后，Engine 单元及业务回归通过。
- Code、Engine、TUI、CLI 严格 Clippy、格式与空白检查通过。全 workspace 的 Projects 遗留 Clippy 告警未混入本次修复。
- Python 测试通过；归档检查补充全部生产本地依赖均在 `crates/` 下的约束。
- macOS release 构建、SHA256 / manifest 校验、隔离安装、CLI / doctor、Projects、UI / viewer / VS Code smoke、回滚及卸载通过。
- 并行的另一组 macOS 快捷键未提交修改保留在原工作目录，不纳入本次提交。

## 历史重写

在独立镜像中清除已删除目录及相关文本引用，验证所有可达分支和标签。原始 bundle、远端 ref 清单、并行工作备份及新旧提交映射保存在仓库外部。

强推采用逐 ref 的原始值作为 lease，并要求 atomic 更新；短暂禁用标签发布工作流以免重写旧标签触发重复构建，操作结束后恢复。旧发行附件记录改写前源码 SHA，不作为改写后源码的构建证明。

最终远端、同 SHA 三平台 CI 与新版本发布验收结果见 GitHub 交付任务。未执行三平台 GUI 人工验收。
