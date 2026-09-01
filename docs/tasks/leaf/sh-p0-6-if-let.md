# SH-P0-6 `if let` / `while let` 模式控制流

> **级别**：P0（阻塞前端 PoC） · **风险**：🔴 高 · **状态**：⏳ 规划中 · **归属**：0.2.0-O
> **索引**：[`../self-hosting-p0.md`](../self-hosting-p0.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.15

## 目标
支持 `if let` / `while let` 模式控制流，使解析器可用 `if let Some(tok) = next() { .. }` 处理 `Option` 返回。语言当前**完全缺失**该语法，解析器/类型检查器重写（`lexer`/`parser` 的 `Option` 返回处理）均依赖此项。

## 现状
- `grammar.md` 无 `if let` / `while let` 产生式；基础 `if`/`while`/`let`/`match` 已存在。
- `Option`/`Result` 已支持（`K1` `?` 运算符已实现），但无「模式绑定于条件位置」的语法。

## 风险分解（→ 中/低危）
- **M1（中）** `if let Pat = expr { .. } else { .. }`：解析为 `let Pat = expr; if /* 绑定成功 */ { .. } else { .. }`，绑定成功判定由模式匹配生成，零新增 IR。
- **M2（中）** `while let Pat = expr { .. }`：循环头绑定 + 条件重评估，复用 M1 模式匹配。
- **M3（低）** `if let`/`while let` 嵌套与链（`if let a = .. && let b = ..` 暂不计，先单模式）。
- **L1（低）** 差分对拍 Rust 参考实现，确认语义一致。

## 受影响组件
`rlyeh-parser`（stmt/expr 解析）、`rlyeh-typecheck`（模式 + 条件作用域）、`rlyeh-codegen`（desugar）。

## 验证
- 解析器 `if let Some(tok) = lexer.next() { .. }` 重写通过差分对拍。
- 单元：`if let`/`while let` 处理 `Option` 返回，结果正确。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 复审补遗：从解析器/类型检查器重写依赖中拆出（语言完全缺失）；补齐缺失叶子文档 |