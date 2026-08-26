# S0c `join` + 返回值传递

> **所属阶段**：阶段 S
> **状态**：✅ 已完成
> **依赖**：S0b
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

`Thread::join` 返回值读取。

## 背景

阶段 阶段 S 子任务，详见 阶段详情文档 [`stages/S.md`](../../stages/S.md)。

## 技术细节

`Thread::join` 返回值槽读取（pthread_join i64*）；启动失败映射 `IoError`（M1b）。

## 验证

`thread_test.rs`（join 返回值）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
