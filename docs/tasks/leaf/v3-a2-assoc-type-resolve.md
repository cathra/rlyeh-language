# V3-A2：typecheck — 关联类型定义 + `Self::Item` 投影

> **所属任务**：[V3 Iterator 关联类型 + 适配器迁移](../v3-iterator-adapters.md)（由 V3-A 细分子任务）
> **状态**：✅ 已完成（U2 基础 + 核实通过，2026-08-27）
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

## 实施情况（U2 已完成，2026-08-27 核实）

- **collect.rs `collect_trait`**：`TraitDef { assoc_types: t.types.clone() }` 已从 AST 填充（collect.rs 150）。
- **collect.rs `collect_impl`**：解析 `type Item = Concrete;` 进 `assoc_types`，方法签名中 `Self::Item` 替换（collect.rs 192-201）。
- **resolve.rs `resolve_ast_type`**：`Self::Item` 在 impl 上下文查 `ctx.assoc_types` 替换；trait 声明上下文退化为占位 `Generic("Self::Item")`（resolve.rs 62-70）。
- **context.rs**：`assoc_types: HashMap<String, Type>`（上下文映射）。
- **generic.rs**：泛型实例化时 `Self::Item` 经 `assoc_types` + `substitute` 替换（generic.rs 306-316）。

## 验证

- [x] 自定义 trait（非 Iterator）声明 `type Item;` 可 typecheck（临时 `MyIter` 测试通过）。
- [x] impl 内 `Self::Item` 解析为具体类型（`type Item = i64;` + `next() -> Option<i64>` 正常）。
- [x] `Option<Self::Item>` 返回值经 impl 具体化正常工作（输出 "got 1"）。
- [ ] 未定义的关联类型访问报错（可补充 compile-fail 测试）。
- [x] 全量回归不回归。

## 为什么是中风险

复用 U2（assoc_types 字段）+ U4（`F::Output` 投影）既有机制，改动集中在 typecheck 两处，但涉及 trait/impl 解析路径，需确保 `Self` 映射正确。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由 V3-A（高风险）细化拆分而来 |
| 2026-08-27 | 核实 U2 已完整实现（collect_trait/collect_impl/resolve/generic 链路）+ 自定义 trait 关联类型用例通过 |
