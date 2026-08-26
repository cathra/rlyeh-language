# Y4 锁 guard 完整 + Channel 泛型化

> **所属阶段**：阶段 Y
> **状态**：📋 规划
> **依赖**：U3
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`Mutex<T>`/`RwLock<T>` 泛型化 + guard + `Channel<T>` 泛型化 + 有界队列。

## 背景

阶段 阶段 Y 子任务，详见 阶段详情文档 [`stages/Y.md`](../../stages/Y.md)。

## 技术细节

`Mutex<T>`/`RwLock<T>` 泛型化（目标签名 `lock(&self) -> MutexGuard<T>`，替代裸 lock/unlock + lock_guard() 命名特判）；`RwLockWriteGuard`/`RwLockReadGuard`；`Deref`/`DerefMut` 语义（`*guard` 解引用访问数据）；`Channel<T>` 泛型化、`bounded_channel(capacity)` 有界队列、`SendError<T>`/`RecvError`/`TryRecvError` 错误类型、`Arc<LockFreeQueue>`。

## 验证

`mutex_generic.{rlyeh,out}`（泛型 Mutex + Deref guard + bounded Channel + 错误类型）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
