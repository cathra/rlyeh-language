# U2 联合 desugar 与 match 收窄

> **所属**：受限制的类型联合（规划 [`type-union.md`](../type-union.md)）
> **状态**：🔧 待实现
> **阶段**：U2（desugar + 收窄）

## 目标

联合值 desugar 为匿名 `EnumDef`；`match` 支持类型臂（type arm）收窄。

## 技术细节

- 联合 → 生成匿名 `EnumDef`（每个成员一个携带该类型的变体 + tag 槽），注入类型环境；复用现有 enum codegen。
- `check_match` 支持类型臂：`match u { i64 => .., String => .. }` 按成员类型分支（复用 tag 比较 + `FieldGet` 提取）。
- 可选 `x is T` 类型守卫（MVP 可仅 `match`）。

## 验收

- [ ] `match` 对联合值按成员类型收窄编译通过。
- [ ] 未收窄禁止直接运算/方法（typecheck 报错）。
- [ ] 与现有 enum/`Option`/`Result` 无回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-30 | 由联合规划细化为叶子 |
