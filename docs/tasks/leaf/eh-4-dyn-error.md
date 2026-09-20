# EH-4 泛型错误类型 `DynError`（anyhow 式便捷）

> **级别**：P2 · **风险**：🟠 中 · **状态**：🟡 待办 · **归属**：0.2.0-AA
> **索引**：[`../rfc/error-handling.md`](../rfc/error-handling.md) §4.4 · **关联**：`eh-2-error-source.md`、`m2a-error-protocol.md`

## 目标
提供统一的错误抽象，降低库间错误类型摩擦：
- `DynError = Box<dyn Error>` 等价类型（或带所有权的 `OwningError`）。
- `Result<T>` 别名（= `Result<T, DynError>`）。
- 便捷构造宏（如 `bail!`/`ensure!`）。

## 现状
- 仅有具体错误类型 `IoError`/`TimeoutError`；无泛型/`dyn` 错误基类（RFC §1.4）。
- `dyn Error` 可行性依赖 protocol object 支持（参考 `k4-gc`/`h4-dyn-protocol`）。

## 风险分解
- **M1（中）** `DynError` 类型定义 + 与 `Error` protocol 的 `Box<dyn Error>` 装箱/拆箱。
- **M2（低）** `Result<T>` 别名 + 库函数默认返回 `Result<T, DynError>`。
- **M3（低）** `bail!`/`ensure!` 宏（依赖 `i1-declarative-macro`）。

## 受影响组件
`rlyeh-std`（`core/error.rl` 新增）、编译器（protocol object 支持）。

## 验证
- 单元：混合错误类型经 `DynError` 聚合；`?` 在 `Result<T, DynError>` 上下文自动 `From` 转换。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | 评审通过后建叶子 |
