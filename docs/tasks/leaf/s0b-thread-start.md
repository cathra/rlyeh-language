# S0b `Thread::start`

> **所属阶段**：阶段 S
> **状态**：✅ 已完成
> **依赖**：S0a
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

语言侧 `thread` 模块 `Thread::start`。

## 背景

阶段 阶段 S 子任务，详见 阶段详情文档 [`stages/S.md`](../../stages/S.md)。

## 技术细节

`thread` 模块（`spawn` 为保留关键字，方法名取 `start`）；函数指针值按地址整数经 extern i64 形参传递（typecheck 放宽 Fn→I64 + codegen ptrtoint）。

## 验证

`thread_test.rs` + 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
