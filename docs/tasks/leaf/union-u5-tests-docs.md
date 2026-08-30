# U5 联合测试与文档

> **所属**：受限制的类型联合（规划 [`type-union.md`](../type-union.md)）
> **状态**：🔧 待实现
> **阶段**：U5（测试与文档）

## 目标

`tests/run-pass` 联合用例 + `grammar.md`/`semantics.md` 补联合章节。

## 技术细节

- 用例：`i64 | String` 声明/赋值/match 收窄；字段级联合；disjoint 校验报错；受限标量枚举作索引/位运算。
- `docs/grammar.md` 补 `|` 联合类型生产式；`docs/semantics.md` 补联合类型规则与 disjoint 约束。

## 验收

- [ ] 联合用例全绿，与现有 enum/`Option`/`Result` 无回归。
- [ ] 权威规范补联合章节。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-30 | 由联合规划细化为叶子 |
