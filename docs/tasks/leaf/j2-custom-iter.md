# J2 自定义迭代器接入 for

> **所属阶段**：阶段 J
> **状态**：✅ 已完成
> **依赖**：J1
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

接收者有 `next() -> Option<Item>` 方法时可 for 迭代。

## 背景

阶段 阶段 J 子任务，详见 阶段详情文档 [`stages/J.md`](../../stages/J.md)。

## 技术细节

`check_for_iterator` 构造 AST `let mut __for_it = it; loop { match __for_it.next() { Some(__elem) => { let pat = __elem; body }, None => break } }`（复用 check_method_call / check_match 全链路）；配套修复 parser `stmt_terminator` 补 `,`（`match { None => break, }`）。

## 验证

`iterator_for.{rlyeh,out}` 6 输出。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
