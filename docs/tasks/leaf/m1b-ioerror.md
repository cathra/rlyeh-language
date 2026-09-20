# M1b `IoError` 结构

> **所属阶段**：阶段 M
> **状态**：✅ 已完成
> **依赖**：M1a
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

定义错误结构体 `IoError`。

## 背景

阶段 阶段 M 子任务，详见 阶段详情文档 [`stages/M.md`](../../stages/M.md)。

## 技术细节

`struct IoError { kind, message: String }` + 构造器/`kind()`/`message()` 访问器（正式 `Display` protocol 随 Q3，MVP 先用 `message()`）。

## 验证

`tests/run-pass/io_error_type.{rlyeh,out}`（构造/访问器）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
