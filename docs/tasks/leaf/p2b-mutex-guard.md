# P2b `MutexGuard` 接线

> **所属阶段**：阶段 P
> **状态**：✅ 已完成
> **依赖**：P2a
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

`Mutex::lock_guard()` 守卫 + 自动解锁。

## 背景

阶段 阶段 P 子任务，详见 阶段详情文档 [`stages/P.md`](../../stages/P.md)。

## 技术细节

`Mutex::lock_guard()` 返回 `MutexGuard`（`p` 裸指针承载 pthread_mutex_t*）+ 作用域结束自动解锁注入；`RwLockWriteGuard` 目标 API 规划。

## 验证

`mutex_guard.{rlyeh,out}`（guard 持锁期 try_lock=false、块尾/if 分支/多守卫/while 循环体自动解锁）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
