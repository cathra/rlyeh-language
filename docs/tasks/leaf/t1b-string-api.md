# T1b String 目标 API 补齐

> **所属阶段**：阶段 T
> **状态**：✅ 已完成
> **依赖**：J
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

String 目标 API：`chars`/`lines`/`split`/`replace`/`to_uppercase`/`to_lowercase`/`trim`。

## 背景

阶段 阶段 T 子任务，详见 阶段详情文档 [`stages/T.md`](../../stages/T.md)。

## 技术细节

`split`/`replace`/`trim` 已有；新增 `chars`（MVP 字节级——逐字节 i64 列表，目标 `Chars` 迭代器 + UTF-8 码点解码规划）、`lines`（委托 `split(『\n』)`，目标 `Lines` 迭代器规划；`\r\n` 行尾 `\r` 保留）、`to_uppercase`/`to_lowercase`（`to_upper`/`to_lower` 的 API 别名，ASCII 语义）。

## 验证

`string_api.{rlyeh,out}`（chars 字节级、lines 行、to_uppercase/to_lowercase 别名一致）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
