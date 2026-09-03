# SH-P1-6 `?` 运算符经 `From` / `Into` 错误自动转换

> **级别**：P1 · **风险**：🟠 中 · **状态**：🟢 完成 · **归属**：0.2.0-T
> **索引**：[`../self-hosting-p1.md`](../self-hosting-p1.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.20

## 目标
扩展 `?` 运算符在 `Result` 上下文的错误自动 `From::from` 转换，支持分层错误（`IoError` → `CompileError`）经 `?` 透明传播，对齐 Rust `?` 语义。

## 现状
- `?` 已实现（K1）：`match { Ok(v) => v, Err(e) => return Err(e) }`，但**无 `From` 转换**（std-lib.md M2b：`From`/`Into` 可解析但 `-> Self` 未支持）。
- 分层错误编译器（typecheck 多错误类型）重度依赖 `?` + `From`。

## 风险分解（→ 中/低危）
- **M1（中）** `Result` 上下文 `?` 扩展：`Err(e) => return Err(From::from(e))`，查找 `impl From<E> for E2`（依赖 G `Self`/`From` 落地）。
- **M2（中）** `Option` 上下文 `?` 经 `From` 转 `Result` 错误（若目标为 `Result<T, E>`）。
- **M3（低）** 无 `From` impl 时退化为现有 `return Err(e)`，保持向后兼容。
- **L1（低）** 差分对拍：分层错误 `?` 传播。

## 受影响组件
`rlyeh-typecheck`（`?` desugar）、`rlyeh-std`（`From`/`Into` impl）。

## 验证
- 单元：内层返回 `IoError`，外层 `Result<T, CompileError>` 经 `?` 自动 `From` 转换传播。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 复审补遗：从分层错误传播依赖中拆出 |
