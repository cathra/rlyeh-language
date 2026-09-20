# SH-P2-11 结构体 `..` 更新 + 字段简写

> **级别**：P2 · **风险**：🟡 低 · **状态**：🟢 完成 · **归属**：0.2.0-X
> **索引**：[`../self-hosting-p2.md`](../self-hosting-p2.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.24

## 目标
支持结构体字段简写 `Foo { x }` ≡ `Foo { x: x }` 与更新语法 `Foo { field, ..base }`（从 `base` 拷贝其余字段后覆盖），对齐 Rust struct literal 习惯，减少 IR/AST 构造样板。

## 现状
- struct 字面量已支持 `field: expr` 显式形式、`..base` 更新语法（typecheck 在 `check_struct_construct` 注入 `base.field` 拷贝）、字段简写（parser 层 `Foo { x, y: .. }` 展开为 `Foo { x: x, y: .. }`）。
- **已知限制**：纯简写首字段 `Foo { x }`（无显式 `field: value` 前缀）暂不支持——parser 的 `looks_like_struct_ctor` 门控仅对「首字段为 `Ident :`」或 `..base` 触发，用于规避与 `if <cond> { <stmt> }` 的语法歧义（`Name { Ident }` 既可能是结构体构造也可能是省略条件括号后的块）。需显式首字段（如 `Foo { x: 0, y }`）即可使用简写其余字段。

## 风险分解（→ 低危）
- **L1（低）** 字段简写：parser 识别单标识符字段 → desugar 为 `Foo { x: x }`（须位于显式首字段之后）✅。
- **L2（低）** 更新语法 `Foo { a, ..base }`：desugar 为「先逐字段拷贝 `base` 各字段，再覆盖显式字段」（base 须同类型）；嵌套/多重 `..` 报错 ✅。

## 受影响组件
`rlyeh-parser`（struct literal 门控 + 简写展开）、`rlyeh-typecheck`（`check_struct_construct` 注入 `base.field` 拷贝）、`rlyeh-codegen`。

## 验证
- 单元：`let p = Point { x: 1, y: 2 }; let q = Point { x: 3, ..p };` → `q.y == 2`。
- 集成：`tests/run-pass/struct_update_shorthand.rl`（字段简写 + `..base`，输出 10/20/3/2/3/99）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 复审补遗：从 AST 构造样板消减依赖中拆出 |
| 2026-09-20 | 验证 L1/L2 已落地（简写须显式首字段在前、..base 拷贝）；新增 run-pass/struct_update_shorthand.rl；全测试 331/331 |
