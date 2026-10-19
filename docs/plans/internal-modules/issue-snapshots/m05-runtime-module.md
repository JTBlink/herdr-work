<!-- herdr-work-internal-modules:M05 -->
# [HerdrWork][M05] 提取宿主传输与有界进程执行模块

## 目标与范围

创建有真实实现的 herdr-work-runtime，将 Reviewr 原 process-tree/JobObject、管道排空及 Herdr socket/pipe 传输迁入，并更新 Tasks/Code/Review 调用点。接口接收显式配置及截止/取消，不推断当前执行或任务。

## 实现位置

原 Reviewr proc/herdr/herdr_socket、Projects 宿主调用与 Engine host，目标 crates/herdr-work-runtime。

## 验收标准

- [ ] 真实超时、后代进程、管道未关闭、取消和回执异常经私有进程/宿主 fixture 验证。
- [ ] Code 不再为了执行 Git 子进程依赖 Review；Runtime 不依赖业务模块或 TUI，无依赖环。
- [ ] 固定 endpoint 和目标身份不被最近焦点替代；传输结果可区分确定未送达与结果不明，保留现有宿主竞态说明。
- [ ] 原实现/测试迁移可追踪，非空转发壳、不新增通用插件容器。

## 父规格与依赖

父规格：#17。直接阻塞：#21。

## 共同约定

复用已导入的原实现，保留许可证、版权与来源。macOS 本机验证，Linux/Windows 以同一源码 SHA 的 GitHub CI 为准；归档任务需真实内容与 SHA256 校验。ready-for-agent 仅表示描述充分，仍须先完成阻塞项。旧名称与旧插件数据兼容不在产品范围；不自动删除用户个人文件，不操作用户已有 pane 或启动真实 Agent。
