<!-- herdr-work-internal-modules:M08 -->
# [HerdrWork][M08] 内聚审查草稿、批次与发送恢复

## 目标与范围

将现有 Engine 草稿/批次持久化和原 Reviewr 评论格式化/发送语义归入 herdr-work-review。Review 依赖 Core/Code/Runtime，Engine 保留跨模块执行选择与联动，UI 只发动作并展示结果。

## 实现位置

Engine review.rs/host.rs 与原 Reviewr model/export/herdr；目标 herdr-work-review、Runtime 与 Engine 编排。

## 验收标准

- [ ] 固定快照、评论锚点/版本、执行绑定、路径实例变化和晚到异步结果检查经产品接口验证。
- [ ] 发送中编辑评论、切换任务、聚焦失败、回执不明、重启后 Sending→Unknown 和禁止自动重发均保持。
- [ ] 原格式化和传输仍实际调用；私有宿主 fixture 验证目标与文本，不能用恒定成功或重写逻辑替代。
- [ ] 渲染/主题/终端/原插件 App 不属于 Review 业务接口；无 Review→Engine 反向依赖或 Code→Review 环。

## 父规格与依赖

父规格：#17。直接阻塞：#23、#24。

## 共同约定

复用已导入的原实现，保留许可证、版权与来源。macOS 本机验证，Linux/Windows 以同一源码 SHA 的 GitHub CI 为准；归档任务需真实内容与 SHA256 校验。ready-for-agent 仅表示描述充分，仍须先完成阻塞项。旧名称与旧插件数据兼容不在产品范围；不自动删除用户个人文件，不操作用户已有 pane 或启动真实 Agent。
