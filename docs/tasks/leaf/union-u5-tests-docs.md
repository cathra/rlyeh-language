# U5 联合测试与文档

> **所属**：受限制的类型联合（规划 [`type-union.md`](../type-union.md)）
> **状态**：✅ 已完成（2026-08-31；U1–U4 全部落地 + 规范与用例齐备，全量 194 用例零回归）
> **阶段**：U5（测试与文档）

## 目标

`tests/run-pass` 联合用例 + `grammar.md`/`semantics.md` 补联合章节。

## 技术细节

- 用例：`i64 | String` 声明/赋值/match 收窄；字段级联合；disjoint 校验报错；受限标量枚举作索引/位运算。
- `docs/grammar.md` 补 `|` 联合类型生产式；`docs/semantics.md` 补联合类型规则与 disjoint 约束。

## 验收

- [x] **权威规范补联合章节**（`grammar.md` §2.4、`semantics.md` §8.4、`CODEBUDDY.md`）。
- [x] 与现有 enum / `Option` / `Result` 无回归（194 用例全通过）。
- [x] 联合用例：`union_basics`（声明 / 构造 / `match` 类型臂收窄）、
      `enum_discriminant`（显式判别式）。
- [x] 字段级联合用例（`struct_field_union.rl` + 两个 compile-fail，U4 已落地）。
- [x] 受限标量枚举作索引 / 位运算用例（`enum_scalar_context.rl` + `enum_scalar_storage.rl`，U3 核心项已落地）。

## 已实现（2026-08-30）

| 位置 | 改动 |
|------|------|
| `docs/grammar.md` §2.4 | 类型联合 `T \| U` 语法（互不相交约束、优先级规则、与闭包 `\|` 的歧义说明）+ 枚举显式判别式 `Variant = 42` |
| `docs/semantics.md` §8.4 | 新增「类型联合语义」：表示（匿名 enum）、disjoint 三类规则表、构造（协变 + 统一堆分配的原因）、收窄（类型臂 + 前置精确匹配的必要性）、优先级、MVP 限制 |
| `CODEBUDDY.md` | 补联合语法示例（含 disjoint 非法样例）与显式判别式，与权威规范一致 |
| `tests/run-pass/union_basics.{rl,out}` | 联合端到端用例（构造 + 类型臂收窄，两种成员） |
| `tests/run-pass/enum_discriminant.{rl,out}` | 显式判别式用例（判别值与序号不一致时仍正确） |
| `docs/semantics.md` §8.5 | 新增「受限标量枚举语义（U3 ✅）」：判定 / 单标量存储 / 整数上下文放宽 / 反向禁止 / 匹配与存储交互 / 显式判别式；修正原 §8.4「U3 尚未实现」过时条目 |
| `docs/grammar.md` §2.4 | 枚举条目由「仅显式判别式」扩写为「受限标量枚举核心项」：单标量存储 + 整数上下文（索引 / 位运算 / 双向比较 / 值即整数） |
| `tests/run-pass/enum_scalar_context.{rl,out}` | 标量枚举整数上下文：数组索引 / 赋值整数 / 整数比较 / 位运算 / match 收窄 |
| `tests/run-pass/enum_scalar_storage.{rl,out}` | 标量枚举存储与布局交互：结构体字段 / 数组元素 / 跨函数返回 / 表达式 match / 联合成员 / 双向整数比较 |
| `tests/compile-fail/enum_scalar_no_int_to_enum.rl` | 反向禁止：整数 → 标量枚举赋值报类型不匹配 |
| `tests/run-pass/struct_field_union.{rl,out}` | U4 字段级联合：字面量构造 / 字段读取 + 类型臂收窄 / 字段赋值（`=`）/ 多字段联合，与值级联合语义一致 |
| `tests/compile-fail/struct_field_union_disjoint.rl` | U4 字段联合成员重叠（`i64 | isize`）报 `UnionMembersNotDisjoint` |
| `tests/compile-fail/struct_field_union_unnarrowed.rl` | U4 字段联合未收窄即运算（`s.id + 1`）报 `expected a numeric type` |
| `tests/run-pass/union_field.rl` | U4 字段级联合同场景早期用例（2026-08-30）：字面量构造 + 字段写入（`c.id = 7` / `c.id = String`）经 `match` 类型臂收窄；与 `struct_field_union.rl` 覆盖重合，保留作回归 |

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-30 | 由联合规划细化为叶子 |
| 2026-08-30 | 补齐权威规范：`grammar.md` §2.4 / `semantics.md` §8.4 / `CODEBUDDY.md` 的联合章节与显式判别式；新增 `union_basics`、`enum_discriminant` 两用例；`rlyeh test` 187 用例零回归 |
| 2026-08-31 | U4 字段级联合完成：新增 `struct_field_union`（run-pass，多场景）+ `struct_field_union_disjoint` / `struct_field_union_unnarrowed`（compile-fail 两例）；`CHANGELOG.md` 补 U4 条目；全量 194 用例零回归 |
