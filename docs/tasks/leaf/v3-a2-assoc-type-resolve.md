# V3-A2：typecheck — 关联类型定义 + `Self::Item` 投影

> **所属任务**：[V3 Iterator 关联类型 + 适配器迁移](../v3-iterator-adapters.md)（由 V3-A 细分子任务）
> **状态**：📋 规划
> **风险**：中（复用 U2/U4 已有机制）
> **依赖**：V3-A1
> **权威来源**：`rlyeh-typecheck`（`TraitDef.assoc_types`、`resolve_ast_type`、U2 ✅ / U4 `F::Output`）

## 目标

typecheck 解析 trait 内关联类型定义，并识别 `Self::Item` 关联类型投影（复用 U4 已支持的 `F::Output` 模式）。

## 背景

`TraitDef.assoc_types` 字段已有（U2 ✅），但 trait 内 `type` 成员未从 AST 填充。`resolve_ast_type` 已能处理 `F::Output` 投影（U4），需扩展到 trait 内的 `Self::Item`。

## 改动范围

- **collect_trait**：从 `AstTraitDecl.types`（V3-A1 载体）填充 `TraitDef.assoc_types`。
- **resolve_ast_type**：识别 `Self::Item`（在 trait impl 上下文把 `Self` 映射到目标类型，查 assoc_types）。
- **投影检查**：关联类型在使用前已定义、类型匹配。

## 验证

- [ ] 自定义 trait（非 Iterator）声明 `type Item;` 可 typecheck。
- [ ] impl 内 `Self::Item` 解析为具体类型。
- [ ] 未定义的关联类型访问报错。
- [ ] 全量回归不回归。

## 为什么是中风险

复用 U2（assoc_types 字段）+ U4（`F::Output` 投影）既有机制，改动集中在 typecheck 两处，但涉及 trait/impl 解析路径，需确保 `Self` 映射正确。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由 V3-A（高风险）细化拆分而来 |
