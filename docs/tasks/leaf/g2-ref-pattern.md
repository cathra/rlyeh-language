# G1 收尾 `ref`/`ref mut` 模式

> **所属阶段**：阶段 G
> **状态**：✅ 已完成
> **依赖**：G1
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

`match` 臂与 `let ref x = e;` 的引用绑定模式。

## 背景

阶段 阶段 G 子任务，详见 阶段详情文档 [`stages/G.md`](../../stages/G.md)。

## 技术细节

parser 早已产出 `AstPattern::Ref(inner, is_mut)`；typecheck `check_pattern` 原忽略 ref 前缀——改为递归检查内层后包装 `HirExpr::Ref { expr, is_mut, pointee }`，绑定变量引用化为 `Type::Ref(T, mutability)`；`let ref [mut] x = e;` 支持。语义注意（与 Rust 差异）：Rlyeh match desugar 先拷贝匹配值到临时槽，ref 绑定指向拷贝。

## 验证

`ref_pattern.{rlyeh,out}` 8 输出 + 全量回归绿。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
