# U1 联合类型层

> **所属**：受限制的类型联合（规划 [`type-union.md`](../type-union.md)）
> **状态**：🔧 待实现
> **阶段**：U1（类型层）

## 目标

新增 `Type::Union(Vec<Type>)`；`parser/src/ty.rs` 支持 `|` 链；`resolve_ast_type` 构建 Union 并做 disjoint 校验。

## 技术细节

- `crates/rlyeh-typecheck/src/types.rs`：`enum Type` 新增 `Union(Vec<Type>)`；`Display` 增加 `Union(ts) => ts.join(" | ")`。
- `crates/rlyeh-parser/src/ty.rs`：primary 类型后遇 `|` 合并为 `AstType` 联合（新增 `AstType::Union` 或复用 `Path` 链——建议新增 `AstType::Union(Vec<AstType>)`）。
- `crates/rlyeh-typecheck/src/check_expr/resolve.rs`：`AstType::Union` → `Type::Union(members)` + disjoint 校验（重复成员合并/报错；`&T | &mut T`、`i64 | isize` 等重叠报 `UnionMembersNotDisjoint`）。
- `compatible_with`：收窄前不允许运算（落 `_` 默认不等）；`field_scalar_of`：按最大成员尺寸（落 `_` 默认 `Int`/Ptr）。

## 验收

- [ ] `let x: i64 | String` 在类型层可表示。
- [ ] 重叠成员报 `UnionMembersNotDisjoint`。
- [ ] `cargo check -p rlyeh-typecheck` 通过。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-30 | 由联合规划细化为叶子 |
