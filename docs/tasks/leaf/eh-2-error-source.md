# EH-2 `Error::source` 真实根因链 + 全错误类型补齐

> **级别**：P2 · **风险**：🟠 中 · **状态**：✅ 已完成（M1/M2 已落地并验证；M3 `kind()` 可选未做） · **归属**：0.2.0-AA
> **索引**：[`../rfc/error-handling.md`](../rfc/error-handling.md) §4.2 · **关联**：`m2a-error-protocol.md`、`m2b-error-convert.md`、`y6-*`

## 目标
将 `Error::source()` 从 `None` 占位升级为真实根因链；为所有 std 错误类型（`IoError`/`TimeoutError` 等）补齐 `source()`；可选补充 `kind()` 分类方法。

## 现状
- `Error` protocol 已落地 `message()`（M2a ✅）与 `source() -> Option<&dyn Error>`（Y6a 起步 → **P7d-1 升级为真实错误链**，2026-08-29）：利用 P4 的 `&dyn Error` 上转型，链可由 `source()?.source()` 逐层追溯，而非字符串拷贝。
- `io/error.rl` 的 `impl IoError: Error::source()` 返回 `Option::None`——**属预期**：`IoError` 不包装底层 `Error` 对象（其分类由 `kind: IoErrorKind` 表达），故无根因可指；真实链由包装型错误（持有底层错误引用字段）承载。
- `future/error.rl` 的 `impl TimeoutError: Error` 亦补齐 `message()`/`source()`。
- `IoError` 已含 `kind: IoErrorKind`（M1b），可作为可选的 `kind()` 基础（未做，非阻塞）。

## 风险分解
- **M1（中）** `IoError::source()` 返回内部 `Option<&dyn Error>`（如包装底层 OS 错误时填充）。✅ 能力落地（返回 `Option<&dyn Error>`；`IoError` 本身无包装源故为 `None`）。
- **M2（低）** `TimeoutError` 及后续错误类型补齐 `source()` / `message()`。✅ 已补齐。
- **M3（低）** 可选 `kind()` 分类方法（`IoErrorKind` 已具备）。📋 未做（可选）。

## 受影响组件
`rlyeh-std`（`io/error.rl`、`future/error.rl` 等错误类型）、`Error` protocol 定义。

## 验证
- `tests/run-pass/error_source.rl`（+ `.out`）：底层错误 `source() = None`；包装错误 `source()` 返回底层 `&dyn Error` 且 `.message()` 取回真实 message；链可遍历至链尾（输出 `1` / `file not found` / `load failed` / `3`）。✅ 通过。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | 评审通过后建叶子 |
| 2026-09-21 | 复核确认 M1/M2 已落地（`source() -> Option<&dyn Error>` 真实链 + IoError/TimeoutError 补齐，`error_source.rl` 验证通过）；状态置 ✅（M3 `kind()` 可选未做）；修正 `std-lib.md` §12 过时标注 |
