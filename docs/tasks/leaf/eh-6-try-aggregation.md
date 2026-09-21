# EH-6 错误聚合 / `try` 块

> **级别**：P3 · **风险**：🟠 中 · **状态**：🟢 部分完成（M2 / M3 ✅ 2026-09-21；M1 `try` 块 ⏳ 待办） · **归属**：0.2.0-AA
> **索引**：[`../rfc/error-handling.md`](../rfc/error-handling.md) §4.6 · **关联**：`eh-4-dyn-error.md`

## 目标
- `try` 块：块内 `?` 不立即返回，而是聚合首个错误（或 `Vec` 累积全部错误）后统一处理。
- 迭代器错误累积：`Vec<Result<T,E>>` → `Result<Vec<T>, E>`（类 `collect`/`try_collect`）。

## 现状（2026-09-21 落地）
- `?` 仅支持**早返回**（`check_question` 合成 `match … { Err(e) => return Err(e) }`，硬编码到函数边界）；无块级聚合机制。

## 风险分解与落地
- **M1（中）`try` 块 desugar（`Result<_, E>` 收集到隐藏累加器）** ⏳ **未落地**
  - **阻塞分析**：
    1. `try` **不是关键字**（lexer 无 `Token::Try`），当前可作普通标识符 —— 新增关键字需评估既有代码冲突（std / tests 中作为标识符的使用）。
    2. `?` 的失败臂在 `check_question`（`check_expr/index_enum.rs`）中**合成 AST `return`**，且用 `ctx.current_return_type` 决定 `From` 转换目标 —— 块级聚合需要引入**残差目标（residual target）**上下文：失败臂改为「写入块级累加器 / `break` 到块边界」。
    3. 调用点亦需让 `?` 在 `try` 块内的类型语义从「函数返回类型」切到「块的 `Result` 类型」。
    4. 若采用 `loop { … break <value> }` 实现：typecheck 已有 `ExprKind::Break(Some(e))` 分支，但 **HIR→LIR→codegen 的 `break <value>` 通路未验证**（现有用法均为 `Break(None)`）。
    5. 「隐藏函数 + 捕获变量作参数」方案**不可行**：闭包捕获外部变量尚未支持（H3），`try` 块内引用外层变量是常态。
  - 建议实施顺序：先补 `break <value>` 的 codegen 并对拍，再做 `try` 关键字 + 残差上下文。
- **M2（中）迭代器 `try_collect` / `Vec` 错误累积** ✅
  - `rlyeh-std/rlyeh/core/module.rl` 新增 `impl<T, E> Vec<Result<T, E>>`：
    - `try_collect(self) -> Result<Vec<T>, E>`：**首错短路**（对齐 Rust `collect::<Result<Vec<T>, E>>()`）；
    - `collect_errors(self) -> Vec<E>`：忽略 `Ok`，按原序累积**全部**错误。
  - 依赖嵌套泛型 self 类型 `impl<T, E> Vec<Result<T, E>>`（EH-3 期间修复）。
  - `try_for_each` 未落地（需接收 `Result<T, E>` 的函数值，价值低于前两者）。
- **M3（低）与 `DynError`（eh-4）组合：聚合为 `Vec<DynError>`** ✅
  - `Vec<Result<T, DynError>>::try_collect()` / `::collect_errors()` 直接可用（`DynError` 为具体 struct），异质错误统一聚合后逐条 `message()`。

## 受影响组件
`rlyeh-std`（`core/module.rl` 新增 `impl<T, E> Vec<Result<T, E>>`）；（M1 待办将涉及 `rlyeh-lexer` / `rlyeh-parser` / `rlyeh-ast` / `rlyeh-typecheck` / `rlyeh-codegen`）。

## 验证
- `tests/run-pass/eh_aggregate_errors.rl`（+ `.out`，12 行）：
  - `try_collect` 全 Ok（`len = 3`）/ 首错短路（`is_err = 1`、错误值 `bad`）；
  - `collect_errors` 多错聚合（`len = 2`，逐条 `e1` / `e2`）；
  - 空集合（`try_collect().unwrap().len() = 0`、`collect_errors().len() = 0`）；
  - M3：`Vec<Result<i64, DynError>>` 的 `try_collect().is_err()` 与 `collect_errors()` 逐条 `message()`（`entity not found` / `io error`）。
- 全量 `rlyeh test tests`：**343/343 通过**（本轮新增 1 例）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | 评审通过后建叶子 |
| 2026-09-21 | **M2+M3 落地**：`impl<T, E> Vec<Result<T, E>>` 的 `try_collect`（首错短路）/ `collect_errors`（全错聚合），与 `DynError` 组合验证；新增 `tests/run-pass/eh_aggregate_errors.{rl,out}`（12 行），全量 343/343。**M1 `try` 块登记为 ⏳ 待办**，附 5 条阻塞分析（`try` 非关键字 / `?` 硬编码函数级 `return` 需残差上下文 / `break <value>` codegen 未验证 / 隐藏函数方案受阻于闭包捕获不支持 / 建议先补 `break <value>` 通路） |
