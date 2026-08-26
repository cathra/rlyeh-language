# P3 `Condvar`/`Barrier`

> **所属阶段**：阶段 P
> **状态**：✅ 已完成
> **依赖**：P2b、S0
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

条件变量与屏障。

## 背景

阶段 阶段 P 子任务，详见 阶段详情文档 [`stages/P.md`](../../stages/P.md)。

## 技术细节

`Condvar::wait/notify_one/notify_all`（与 Mutex 配对）+ `Barrier::new(count)/wait`（`PTHREAD_BARRIER_SERIAL_THREAD` 领头线程语义简化）。

## 验证

多线程验证（S0 线程支持后）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
