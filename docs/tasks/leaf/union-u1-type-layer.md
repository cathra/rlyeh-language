# U1 联合类型层

> **所属**：受限制的类型联合（规划 [`type-union.md`](../type-union.md)）
> **状态**：✅ 已完成（2026-08-30；`Type::Union` + `|` 链解析 + disjoint 校验三类规则均验证）
> **阶段**：U1（类型层）

## 目标

新增 `Type::Union(Vec<Type>)`；`parser/src/ty.rs` 支持 `|` 链；`resolve_ast_type` 构建 Union 并做 disjoint 校验。

## 技术细节

- `crates/rlyeh-typecheck/src/types.rs`：`enum Type` 新增 `Union(Vec<Type>)`；`Display` 增加 `Union(ts) => ts.join(" | ")`。
- `crates/rlyeh-parser/src/ty.rs`：primary 类型后遇 `|` 合并为 `AstType` 联合（新增 `AstType::Union` 或复用 `Path` 链——建议新增 `AstType::Union(Vec<AstType>)`）。
- `crates/rlyeh-typecheck/src/check_expr/resolve.rs`：`AstType::Union` → `Type::Union(members)` + disjoint 校验（重复成员合并/报错；`&T | &mut T`、`i64 | isize` 等重叠报 `UnionMembersNotDisjoint`）。
- `compatible_with`：收窄前不允许运算（落 `_` 默认不等）；`field_scalar_of`：按最大成员尺寸（落 `_` 默认 `Int`/Ptr）。

## 验收

- [x] `let x: i64 | String` 在类型层可表示（签名 / 类型注解均可，Display 为 `i64 | String`）。
- [x] 重叠成员报 `UnionMembersNotDisjoint`，三类规则均验证：
      - 重复成员 `i64 | i64` →「重复成员」
      - 引用可变性重叠 `&i64 | &mut i64` →「引用可变性重叠（`&T` 与 `&mut T`）」
      - 平台相关重叠 `i64 | isize` →「数值类型互通（宽度 / 平台相关重叠）」
- [x] `cargo check` 全工作区通过；`rlyeh test` 185 用例零回归。

> 未收窄的联合**禁止赋值 / 运算**（`compatible_with` 落 `_ => self == other` 默认不等，
> 报 `expected 'i64 | String', found 'i64'`）——符合规划 §3.1「必须 `match` 收窄后才能使用」，
> 赋值与收窄能力由 U2（desugar + 类型臂）提供。

## 关键坑（后续接手必读）

1. **`|` 的解析优先级**：`&T | &mut U` 须解析为 `(&T) | (&mut U)`。最初让 `&` / `*` 的
   内层递归调 `parse_type`（会收集联合），结果解析成 `&(T | &mut U)`——其成员 `i64`
   与 `&mut i64` 确实不相交，导致引用重叠**漏检**。现拆分为 `parse_type`（收集联合）与
   `parse_primary_type`（不含联合），前缀构造内层走后者；括号 / 泛型实参 / 元组 / 数组 /
   fn 签名仍走 `parse_type`，故 `Vec<i64 | String>` 等嵌套联合可正常表达。
2. **闭包参数注解的 `|` 歧义（规划 §9 误判）**：规划称"类型上下文无歧义"，实际
   `|x: i64| x + 1` 中注解后的 `|` 是**参数列表结束符**，被联合解析误吞后报
   `expected '|', found Plus`（9 个闭包用例回归失败）。修复：闭包参数类型注解改用
   `parse_primary_type`；需联合时写 `|x: (i64 | String)| ..`（括号内仍走 `parse_type`）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-30 | 由联合规划细化为叶子 |
| 2026-08-30 | 实现：`AstType::Union` / `Type::Union` + Display、parser `|` 链解析（拆分 `parse_primary_type` 修正优先级）、`resolve_ast_type` 构建 + `check_union_disjoint` 校验、错误 `UnionMembersNotDisjoint`；`rlyeh-doc` / `rlyeh-fmt` 补 `fmt_type` 分支；修复闭包参数注解的 `|` 歧义；185 用例零回归 |
