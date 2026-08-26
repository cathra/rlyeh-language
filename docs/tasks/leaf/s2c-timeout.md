# S2c `timeout`

> **所属阶段**：阶段 S
> **状态**：✅ 已完成
> **依赖**：S2b
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

Future 超时包装。

## 背景

阶段 阶段 S 子任务，详见 阶段详情文档 [`stages/S.md`](../../stages/S.md)。

## 技术细节

`timeout<T>(duration, &mut fut) -> Result<i64, i64>`（MVP 退化：`TimeoutError` 规划中，超时 `Err(-1)`；超时判定经墙钟 `__rlyeh_clock_monotonic`，返回 -1 退回 clock()；忙等轮询，事件驱动规划随 R1 Poller）。

## 验证

`timeout.{rlyeh,out}`（3 轮后 Ready → Ok(3) + 恒 Pending 50ms → Err(-1)，输出 3/-1）+ `time_test.rs`。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
