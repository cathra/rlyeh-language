# U2 联合 desugar 与 match 收窄

> **所属**：受限制的类型联合（规划 [`type-union.md`](../type-union.md)）
> **状态**：✅ 已完成（2026-08-30；构造 desugar + `match` 类型臂收窄均验证通过，186 用例零回归）
> **阶段**：U2（desugar + 收窄）

## 目标

联合值 desugar 为匿名 `EnumDef`；`match` 支持类型臂（type arm）收窄。

## 技术细节

- 联合 → 生成匿名 `EnumDef`（每个成员一个携带该类型的变体 + tag 槽），注入类型环境；复用现有 enum codegen。
- `check_match` 支持类型臂：`match u { i64 => .., String => .. }` 按成员类型分支（复用 tag 比较 + `FieldGet` 提取）。
- 可选 `x is T` 类型守卫（MVP 可仅 `match`）。

## 验收

- [x] **联合值构造**：`let x: i64 | String = 5;` desugar 为匿名 enum 布局，编译通过。
- [x] **`match` 对联合值按成员类型收窄编译通过**：分支选择（tag 比较）+ payload 按成员
      类型绑定，两种成员（标量 `i64`、聚合 `String`）均验证通过。
- [x] 未收窄禁止直接运算/方法（`compatible_with` 单向：成员 → 联合 ✅，联合 → 成员 ❌）。
- [x] 与现有 enum / `Option` / `Result` 无回归（186 用例全通过）。

## 已实现（2026-08-30）

| 位置 | 改动 |
|------|------|
| `crates/rlyeh-typecheck/src/types.rs` | `field_scalar_of`：`Type::Union` → `FieldScalar::Ptr`（联合值 = 匿名 enum 对象指针，槽 0 tag + 槽 1 payload） |
| `crates/rlyeh-typecheck/src/types.rs` | `compatible_with` 新增两分支：成员 → 联合的**向上转换**（构造，协变）；联合 → 联合（成员集合相同即兼容）。反向（联合 → 成员）不落此分支，故未收窄仍禁止使用 |
| `crates/rlyeh-typecheck/src/check_stmt.rs` | `make_union_ctor()` + `let` 处的构造 desugar：注解为 `A \| B`、init 为某成员时，生成与 `check_variant_construct` 同构的 `Alloc(2)` + `FieldSet(0, tag)` + `FieldSet(1, value)` |

## 设计要点（供后续实现收窄参考）

1. **不注册 `EnumDef`**：联合的匿名 enum 直接内联生成 HIR，避免 `__U0` 之类的变体名
   污染 `variant_index` 全局名空间（该表按变体名索引，重复会互相覆盖）。代价是联合
   没有具名类型，故 `Type::Union` 需常驻类型层（U1 的 Display / disjoint 依赖它）。
2. **统一 `by_value: false`（calloc 堆对象）**：成员可能分别是标量（`i64`）与聚合
   （`String`），若按成员分别决定栈槽 / 堆分配，同一联合的各构造路径判定会不一致
   （`check_variant_construct` 明确要求一致），且栈槽地址存入联合值后传出函数会悬垂。
3. **类型臂收窄已实现**：在 `check_pattern`（`check_expr/index_enum.rs:578`）的
   `AstPattern::Ident` 分支**前置**判定——`pat_ty` 为联合且 `name` 与某成员的
   `to_string()` 相同时，即为类型臂，产出：
   - `cond` = `FieldGet(scrutinee, 0) == 成员下标`（复用现有 tag 比较）；
   - `binds` = `let slot = FieldGet(scrutinee, 1)`，按成员类型绑定 payload（复用现有 `FieldGet`）。

   再经 `check_match_with_scrutinee` 既有的 if-else 链组装，**无新增 codegen**。
   因 `Ident` 模式本为"绑定变量"语义，类型名判定必须**前置**且严格（成员类型名精确匹配），
   否则会误吞普通变量绑定（与 U1 中 `|` 同闭包冲突是同一类教训）。

## MVP 约定与后续项

- **payload 绑定到类型名同名变量**：`match x { i64 => println(i64) }`——臂内以类型名
  作变量名取 payload。这是 MVP 简化；后续可引入 `x @ T` 绑定语法（需 parser 支持）以免
  类型名 / 变量名混用。
- 未收窄的联合值禁止运算 / 方法调用（`compatible_with` 单向），符合规划 §3.1。
- `x is T` 类型守卫为可选项，未实现。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-30 | 由联合规划细化为叶子 |
| 2026-08-30 | 实现：联合值按匿名 enum 布局（`field_scalar_of` → Ptr）、成员→联合的协变、构造 desugar（`make_union_ctor`）；`let x: i64 \| String = 5;` 编译通过；`rlyeh test` 185 用例零回归 |
| 2026-08-30 | 实现：`match` 类型臂收窄（`check_pattern` 的 Ident 分支前置类型名判定 → tag 比较 + payload 绑定）；端到端验证 `i64` 臂取 `5`、`String` 臂取 `.len() = 2`；新增 `tests/run-pass/union_basics.{rl,out}`；`rlyeh test` 186 用例零回归 |
