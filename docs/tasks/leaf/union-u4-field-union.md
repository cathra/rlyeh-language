# U4 字段级联合

> **所属**：受限制的类型联合（规划 [`type-union.md`](../type-union.md)）
> **状态**：✅ 已完成（2026-08-30；声明 / 构造 / 读取收窄均验证，两种成员通过）
> **阶段**：U4（字段级联合）

## 目标

`struct` 字段类型可写联合（`struct S { id: i64 | String }`），即字段级匿名联合。

## 技术细节

- `struct` 字段类型解析支持 `Union`（复用 U1 类型层）。
- 字段读写遵循 U2 的 match 收窄规则。
- 布局：字段存为匿名 `EnumDef`（tag + payload 槽）。

## 验收

- [x] `struct S { id: i64 | String }` 声明可用。
- [x] 字段读写 + 收窄编译通过（`i64` 与 `String` 两种成员均验证）。
- [x] 与现有 struct 字段无回归（188 用例全通过）。

## 已实现（2026-08-30）

| 位置 | 改动 |
|------|------|
| `crates/rlyeh-typecheck/src/check_expr/construct.rs` | `check_struct_construct` 字段循环：字段类型为 `Type::Union` 且实参为某成员时，用 `make_union_ctor` 包装，使字段槽存联合值（匿名 enum 对象指针）而非裸成员值；并把 `arg_ty` 记为联合类型 |
| `crates/rlyeh-typecheck/src/check_stmt.rs` | `make_union_ctor` 由私有改 `pub(crate)`，供字段构造复用（U2 已实现，本次仅导出） |

说明：字段类型解析（`parse_type` 收集 `\|`、`resolve_ast_type` 构建 `Union` + disjoint 校验）
与字段存储的 `field_scalar_of(Union) -> Ptr` 已分别由 U1、U2 完成，故 U4 只需在**构造**
这一环插入 desugar——这也是「复用优先」的设计收益。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-30 | 由联合规划细化为叶子 |
| 2026-08-30 | 实现：字段级联合**构造**——`check_struct_construct` 构造时 desugar 联合字段（复用 U2 的 `make_union_ctor`）；验证 `S { id: 42 }` 取 `42`、`S { id: String::from("hi") }` 取 `.len() = 2`；新增 `tests/run-pass/union_field.{rl,out}`；188 用例零回归 |
| 2026-08-30 | 实现：字段级联合**写入**——`check_expr/mod.rs` 赋值处理在生成 `FieldSet` 前 desugar 联合右值（仅 `=`，复合赋值不适用）；验证写入后判别值切换（`i64` tag=0 → `String` tag=1）与两种成员取值（`7`、`3`）；用例扩展为 4 组断言（`42/2/7/3`）；188 用例零回归 |

**两条路径缺一不可**：构造走 `check_struct_construct`（结构体字面量），赋值走
`check_expr/mod.rs` 的 `FieldSet` 生成处——二者是独立代码路径，只改其一会导致
「构造正常但赋值后收窄读到错误布局」。
