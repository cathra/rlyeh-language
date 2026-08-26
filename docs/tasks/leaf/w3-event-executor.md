# W3 事件驱动 executor

> **所属阶段**：阶段 W
> **状态**：✅ 已完成
> **依赖**：R1
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`block_on` 从忙等轮询改为事件驱动。

## 背景

阶段 阶段 W 子任务，详见 阶段详情文档 [`stages/W.md`](../../stages/W.md)。

## 技术细节

两步完成：① 定时器唤醒——`Context` 携带 `deadline` 槽，`block_on`/`timeout` 据此 `thread::sleep` 到唤醒时刻再轮询非忙等；`future::sleep` 定时器 future。② fd 事件唤醒——`Context` 携带 `fd`/`interest` 槽，`block_on` `Pending` 且 `fd>0` 时构造 `Poller`（poll(2)）注册并等待就绪；`future::wait_fd(fd, interest)` 落地。为 W5 提供真异步底座。

## 验证

`async_sleep.{rl,out}` + `executor_test.rs` 3 用例全绿（sleep 墙钟 23ms/CPU 37us、TcpListener + 延迟 connect 墙钟 35ms/CPU 0.3ms）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
