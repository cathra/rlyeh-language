# SH-P2-11 结构体 `..` 更新 + 字段简写

> **级别**：P2 · **风险**：🟡 低 · **状态**：⏳ 规划中 · **归属**：0.2.0-X
> **索引**：[`../self-hosting-p2.md`](../self-hosting-p2.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.24

## 目标
支持结构体字段简写 `Foo { x }` ≡ `Foo { x: x }` 与更新语法 `Foo { field, ..base }`（从 `base` 拷贝其余字段后覆盖），对齐 Rust struct literal 习惯，减少 IR/AST 构造样板。

## 现状
- 当前 struct 字面量要求 `field: expr` 显式形式（std-lib.md 示例均为 `Point { x: 1, y: 2 }`）；`..` 更新语法缺失（grammar.md 无产生式）。

## 风险分解（→ 低危）
- **L1（低）** 字段简写 `Foo { x }`：parser 识别单标识符字段 → desugar 为 `Foo { x: x }`。
- **L2（低）** 更新语法 `Foo { a, ..base }`：desugar 为「先逐字段拷贝 `base` 各字段，再覆盖显式字段」（base 须同类型）；嵌套/多重 `..` 报错。

## 受影响组件
`rlyeh-parser`（struct literal）、`rlyeh-typecheck`（desugar）、`rlyeh-codegen`。

## 验证
- 单元：`let p = Point { x: 1, y: 2 }; let q = Point { x: 3, ..p };` → `q.y == 2`。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 复审补遗：从 AST 构造样板消减依赖中拆出 |
