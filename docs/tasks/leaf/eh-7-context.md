# EH-7 错误上下文 / 背链附加（可选）

> **级别**：P3 · **风险**：🟢 低 · **状态**：🟡 待办（可选） · **归属**：0.2.0-AA
> **索引**：[`../rfc/error-handling.md`](../rfc/error-handling.md) §4.7 · **关联**：`eh-2-error-source.md`、`eh-4-dyn-error.md`

## 目标
提供 `context()`/`with_context()` 式组合子，在错误传播路径上附加语义上下文（如「读取配置失败：{path}」），形成可读背链。

## 现状
- 无错误上下文附加机制（RFC §2 表中 ❌）。
- 依赖 `source()` 根因链（eh-2）与 `DynError`（eh-4）落地。

## 风险分解
- **M1（低）** `Result<T,E>::context(msg)` 包装为带上下文的 `DynError`。
- **M2（低）** `with_context(fn)` 惰性求值。

## 受影响组件
`rlyeh-std`（`core/error.rl`）、`DynError`（eh-4）。

## 验证
- 单元：嵌套 `context` 后 `source()`/`message()` 输出完整背链。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | 评审通过后建叶子 |
