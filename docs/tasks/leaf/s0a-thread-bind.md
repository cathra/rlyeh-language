# S0a 线程绑定

> **所属阶段**：阶段 S
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

driver 注入线程平台内建。

## 背景

阶段 阶段 S 子任务，详见 阶段详情文档 [`stages/S.md`](../../stages/S.md)。

## 技术细节

driver 注入 `__rlyeh_thread_spawn`/`__rlyeh_thread_join`/`__rlyeh_thread_self`（pthread_create/join/self C ABI 封装，线程入口 `i64 (i8*)*`；替代原计划 Rust 绑定层）。

## 验证

`thread_test.rs`（S0：start/join 返回值、双线程并行求和、current 正数断言 3 用例）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
