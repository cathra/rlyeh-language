# I3 集合宏 `arr!`/`vec!`/`map!`

> **所属阶段**：阶段 I
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

parse 期 desugar 集合宏，零新增 IR 节点。

## 背景

阶段 阶段 I 子任务，详见 阶段详情文档 [`stages/I.md`](../../stages/I.md)。

## 技术细节

`arr![a,b]` → `ExprKind::ArrayLit`；`vec![a,b]` → 块表达式 `let mut __vec_N = Vec::with_capacity(n); push(a); ...; __vec_N`（map! 同构 HashMap + insert）；空集合 → `Vec::new()`/`HashMap::new()`。`parse_collection_macro` 元素 token 流包裹 `()` 后经子 Parser from_tokens 解析。关键教训：子 Parser 手动元素循环必须先消费包裹的 `(`。

## 验证

`collection_macros.{rlyeh,out}` 10 输出 + 全量 41 用例 + clippy 0 警告。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
