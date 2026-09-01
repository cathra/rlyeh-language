# SH-P0-5 元组值构造 + 解构（多返回值）

> **级别**：P0（阻塞前端 PoC） · **风险**：🔴 中高 · **状态**：⏳ 规划中 · **归属**：0.2.0-N
> **索引**：[`../self-hosting-p0.md`](../self-hosting-p0.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.14

## 目标
支持元组**值构造** `(a, b, c)`、解构绑定 `let (a, b) = e`、函数**多返回值** `fn f() -> (i64, String)`，使解析器可用 `(token, rest)` 风格重写（当前 std `Channel` 因「未提供多元组返回」退化为 `ChannelPair` 结构体，即此缺口的旁证）。

## 现状（已具备地基）
- parser 已解析元组**类型** `(A, B)` → `AstType::Tuple`；typecheck 已有 `Type::Tuple` 并参与单态化/替换/字段计数（`field.rs:561`）。
- `ExprKind::Unit` / `Type::Unit`（单元 `()`）已支持。
- **缺失**：元组值字面量解析、解构模式（`index_enum.rs:765` 报 `元组 / 结构体模式在 MVP 阶段` Unsupported）、函数多返回与 codegen 发射。

## 风险分解（→ 中/低危）
- **M1（中）** 元组字面量 `(a, b, c)` 解析 + HIR/MIR/LIR：复用现有聚合槽布局，按位置命名字段 `f0/f1/...`（与枚举负载字段同构），codegen 发射为 N 槽聚合。
- **M2（中）** 解构绑定 `let (a, b) = e` / `let (a, _, c) = e`：pattern 层 `AstPattern::Tuple` 支持标识符/`_`/嵌套，逐字段 `Assign` 到临时再绑定。
- **M3（中）** 函数多返回值 `fn f() -> (i64, String)` + 调用点 `let (x, y) = f()`：返回聚合经现有多槽返回机制；尾表达式/显式 `return (x, y)` 复用 M1/M2。
- **M4（低）** 元组参与 `for (k, v) in map` 既有特判一致化（当前 `iter.rs:549` 已特判二元元组模式）。
- **L1（低）** 差分测试（K 阶段）对拍 Rust 参考实现，确认内存布局/ABI 一致。

## 受影响组件
`rlyeh-lexer` / `rlyeh-parser`（PoC 重写）、`rlyeh-typecheck`（pattern/expr）、`rlyeh-codegen`（聚合发射）。

## 验证
- Rlyeh 侧 `(tok, rest)` 风格重写 lexer/parser 主体，经 Rust driver 编译 + 差分对拍 token/AST 一致。
- 单元：多返回函数 `fn split() -> (i64, String)` + 解构接收，结果正确。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 复审补遗：从评估报告漏判项中拆出（类型层已就绪，值/解构/多返回缺失） |
