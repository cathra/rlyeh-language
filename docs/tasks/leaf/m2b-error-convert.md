# M2b 错误转换约定（`From`/`Into` 验证）

> **所属阶段**：阶段 M
> **状态**：✅ 已完成
> **依赖**：M2a
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

验证 `From`/`Into` 泛型 protocol，回退窄化转换。

## 背景

阶段 阶段 M 子任务，详见 阶段详情文档 [`stages/M.md`](../../stages/M.md)。

## 技术细节

验证结论：泛型 protocol 声明可解析（`protocol From<T>`），但 protocol 方法返回 `Self` 未支持（typecheck `undefined type Self`），parser 无 where 子句（blanket impl 不可行）。**MVP 回退**：`IoError::from_kind(kind)` 窄化转换入口（kind → 默认 message），语义等同 `From::from`。

## 验证

`io_error_type.{rlyeh,out}` + 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
