# P1a 无界队列 Channel

> **所属阶段**：阶段 P
> **状态**：✅ 已完成
> **依赖**：K3、M
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

`sync::Channel` 无界队列通道。

## 背景

阶段 阶段 P 子任务，详见 阶段详情文档 [`stages/P.md`](../../stages/P.md)。

## 技术细节

`sync::Channel`（Mutex + Condvar 队列，`Rc<Channel>` 共享；`send`/`recv`/`try_send`/`try_recv`/`close`；无界队列 send 恒不阻塞，recv 空挂起）；纯 Rlyeh 侧 pthread extern FFI（`sync/module.rl`），MVP 元素限 `i64`、无界队列、queue 只增（head 单调推进）。

## 验证

`channel.{rlyeh,out}`（P1：同线程 send/recv）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
