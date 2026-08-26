# M1a `IoErrorKind` 枚举

> **所属阶段**：阶段 M
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

定义统一错误类型枚举 `IoErrorKind`。

## 背景

阶段 阶段 M 子任务，详见 阶段详情文档 [`stages/M.md`](../../stages/M.md)。

## 技术细节

`enum IoErrorKind { NotFound, PermissionDenied, AlreadyExists, InvalidInput, WouldBlock, TimedOut, Other }`（复用 enum/match ✅，独立可验收）。

## 验证

`tests/run-pass/io_error_type.{rlyeh,out}`（kind 匹配）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
