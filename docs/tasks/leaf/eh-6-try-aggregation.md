# EH-6 错误聚合 / `try` 块

> **级别**：P3 · **风险**：🟠 中 · **状态**：🟢 完成（M1 / M2 / M3 全部 ✅ 2026-09-21；M1 前置 `loop { break <value> }` ✅） · **归属**：0.2.0-AA
> **索引**：[`../rfc/error-handling.md`](../rfc/error-handling.md) §4.6 · **关联**：`eh-4-dyn-error.md`

## 目标
- `try` 块：块内 `?` 不立即返回，而是聚合首个错误（或 `Vec` 累积全部错误）后统一处理。
- 迭代器错误累积：`Vec<Result<T,E>>` → `Result<Vec<T>, E>`（类 `collect`/`try_collect`）。

## 现状（2026-09-21 落地）
- `?` 仅支持**早返回**（`check_question` 合成 `match … { Err(e) => return Err(e) }`，硬编码到函数边界）；无块级聚合机制。

## 风险分解与落地
- **M1（中）`try` 块 desugar（`Result<_, E>` 收集到隐藏累加器）** ✅ **已落地（2026-09-21）**
  - **前置 `loop { break <value> }` 循环值** ✅（见下「前置」行与变更记录）：`let x = loop { break 5 };` 得 `x = 5`。
  - **实现要点（与原「剩余阻塞」逐条对应）**：
    1. `try` 关键字：lexer 新增 `Token::Try`（已核实 std / tests 未将其作标识符，零迁移成本）。AST 新增 `ExprKind::TryBlock(AstBlock)` / `ExprKind::TryBreak(Box<AstExpr>)`；parser `parse_try_block` 前缀分派；stmt 列表补 `TryBlock` 使块尾表达式语义正确。
    2. **残留上下文只需深度计数**：`check_question`（`check_expr/index_enum.rs`）检测 `ctx.try_loop_bases` 非空时，把 `?` 失败臂由 `Return(..)` 改为 `TryBreak(<Err/None>(..))`；错误类型仍取 `current_return_type` 的 `E`，`From` 自动转换逻辑原样复用，**无需类型推断**。
    3. **desugar 形态**：`check_try_block`（`check_expr/ctrl.rs`）将 `try { <stmts>; <tail> }` 组装为 `loop { <stmts>; break <Kind>::<OkVariant>(<tail>) }`（`Kind`/`OkVariant` 取自 `current_return_type`：`Result`→`Ok` / `Option`→`Some`），再 `infer_expr`。
    4. **`try` 块类型显式构造**：`TryBreak` 节点在 `infer_expr_tail` 中降低为 `HIR Break(Some(..))` 但**不登记** `loop_break_types`（避免 Ok 位未定的 `Result<?, E>` 污染块类型）；仅 `break Ok(tail)` 登记 → 循环类型 = `Result<T_tail, ?>`，`T_tail` 具体，`check_try_block` 再以 `current_return_type` 的 `E` 显式组装为 `Result<T_tail, E>`（/`Option<T_tail>`）返回。
    5. **隐式循环越界拦截**：进块时 `ctx.try_loop_bases.push(loop_break_types.len())`，`infer_expr_tail` 的 `Break`/`Continue` 分支检测落在该基准 +1 层的裸跳出即报 `TC016d`（`BreakOutsideLoop`）；嵌套真实循环深度更深（`base+2` 起）不受影响，其 `break`/`continue` 正常。
  - **已知限制**：`try` 须位于返回 `Result`/`Option` 的函数内（否则 `TC025` `Unsupported`）；`try` 内不支持裸 `break`/`continue`（越界拦截）；`TryBreak` 节点仅 typecheck 合成、不参与 desugar 的 actor await 重写（块体已递归覆盖）。
  - 受影响组件：`rlyeh-lexer`（`Token::Try`）、`rlyeh-ast`（`TryBlock`/`TryBreak`）、`rlyeh-parser`（`parse_try_block`）、`rlyeh-desugar`（各 walker 补新节点递归）、`rlyeh-typecheck`（`context.rs::try_loop_bases`、`check_expr/ctrl.rs` 的 `check_try_block` + `TryBlock`/`TryBreak` 分支 + 越界拦截 + `check_question` 残留分流）、`rlyeh-driver`（`token_to_canonical`）、`rlyeh-fmt`/`rlyeh-check`（新节点格式化 / 遍历）、`crates/rlyeh-typecheck/src/error.rs`（`BreakOutsideLoop`）、`tests/run-pass/eh_try_block.{rl,out}`。
- **M2（中）迭代器 `try_collect` / `Vec` 错误累积** ✅
  - `rlyeh-std/rlyeh/core/module.rl` 新增 `impl<T, E> Vec<Result<T, E>>`：
    - `try_collect(self) -> Result<Vec<T>, E>`：**首错短路**（对齐 Rust `collect::<Result<Vec<T>, E>>()`）；
    - `collect_errors(self) -> Vec<E>`：忽略 `Ok`，按原序累积**全部**错误。
  - 依赖嵌套泛型 self 类型 `impl<T, E> Vec<Result<T, E>>`（EH-3 期间修复）。
  - `try_for_each` 未落地（需接收 `Result<T, E>` 的函数值，价值低于前两者）。
- **M3（低）与 `DynError`（eh-4）组合：聚合为 `Vec<DynError>`** ✅
  - `Vec<Result<T, DynError>>::try_collect()` / `::collect_errors()` 直接可用（`DynError` 为具体 struct），异质错误统一聚合后逐条 `message()`。

## 受影响组件
`rlyeh-std`（`core/module.rl` 新增 `impl<T, E> Vec<Result<T, E>>`）；**前置**：`rlyeh-typecheck`（`context.rs::loop_break_types`、`check_expr/ctrl.rs` 的 `While` / `Loop` / `Break` 分支）、`rlyeh-mir`（`lower.rs` 的 `LoopCtx::break_value` / `HirExprKind::Break` / `lower_loop`）、`tests/run-pass/loop_break_value.{rl,out}`；**M1**：`rlyeh-lexer`（`Token::Try`）、`rlyeh-ast`（`TryBlock`/`TryBreak`）、`rlyeh-parser`（`parse_try_block`）、`rlyeh-desugar`（各 walker 补新节点）、`rlyeh-typecheck`（`context.rs::try_loop_bases`、`check_expr/ctrl.rs` 的 `check_try_block` + `TryBlock`/`TryBreak` 分支 + 越界拦截 + `check_question` 残留分流）、`rlyeh-driver`（`token_to_canonical`）、`rlyeh-fmt`/`rlyeh-check`（新节点格式化 / 遍历）、`crates/rlyeh-typecheck/src/error.rs`（`BreakOutsideLoop`）、`tests/run-pass/eh_try_block.{rl,out}`。

## 验证
- `tests/run-pass/eh_aggregate_errors.rl`（+ `.out`，12 行）：
  - `try_collect` 全 Ok（`len = 3`）/ 首错短路（`is_err = 1`、错误值 `bad`）；
  - `collect_errors` 多错聚合（`len = 2`，逐条 `e1` / `e2`）；
  - 空集合（`try_collect().unwrap().len() = 0`、`collect_errors().len() = 0`）；
  - M3：`Vec<Result<i64, DynError>>` 的 `try_collect().is_err()` 与 `collect_errors()` 逐条 `message()`（`entity not found` / `io error`）。
- 全量 `rlyeh test tests`：**343/343 通过**（本轮新增 1 例）。
- 前置（`break <value>`）：`tests/run-pass/loop_break_value.{rl,out}`（10 行：单值 / 多臂汇合 / `break;` → `()` / 嵌套 / 查找惯用法 / 字符串与浮点值 / `while`·`for` 不回退），全量 **345/345 通过**。
- **M1（`try` 块）**：`tests/run-pass/eh_try_block.{rl,out}`（6 行：成功 / 首个 `?` 失败 / 第二个 `?` 失败 / `Option` 成功 / `Option` 失败 / 结果当普通 `Result` 返回），全量 **346/346 通过**。拦截已验：非 `Result`/`Option` 函数内 `try` → `TC025`；`try` 体内裸 `break`/`continue`（直接位于块体）→ `TC016d`；嵌套真实循环的 `break`/`continue` 不受影响。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | 评审通过后建叶子 |
| 2026-09-21 | **M2+M3 落地**：`impl<T, E> Vec<Result<T, E>>` 的 `try_collect`（首错短路）/ `collect_errors`（全错聚合），与 `DynError` 组合验证；新增 `tests/run-pass/eh_aggregate_errors.{rl,out}`（12 行），全量 343/343。**M1 `try` 块登记为 ⏳ 待办**，附 5 条阻塞分析（`try` 非关键字 / `?` 硬编码函数级 `return` 需残差上下文 / `break <value>` codegen 未验证 / 隐藏函数方案受阻于闭包捕获不支持 / 建议先补 `break <value>` 通路） |
| 2026-09-21 | **M1 前置落地：`loop { break <value> }` 循环值**——实测 `let x = loop { break 5 };` 中 `x` 无值（`println(x)` 输出空行、无报错）：`break e` 的值在 **typecheck**（`ExprKind::Loop` 恒 `Type::Never`）与 **MIR**（`lower.rs` 注释「MVP 阶段丢弃」+ `lower_loop` 出口块恒 `Unit`）两处被丢。补齐 `TypeContext::loop_break_types`（循环 break 值类型栈，`while` 亦压栈占位以防误写外层槽）与 `LoopCtx::break_value`（循环体降低前分配承载槽，带值 `break` 写入后跳出，出口块读该槽）。新增 `tests/run-pass/loop_break_value.{rl,out}`（10 行），全量 **345/345**。**阻塞 #4 消除**；其余设计经探针逐一验证并记入上节（残留上下文只需深度计数、desugar 形态、裸 `Result::Ok` 的 `E` 宽松、`?` 残留须用独立 AST 节点、须拦截 `try` 体内裸 `break`/`continue`） |
| 2026-09-21 | **M1 落地：`try { .. }` 错误聚合块**——lexer 新增 `Token::Try`（已核实 std / tests 未将 `try` 作标识符）；AST `TryBlock`/`TryBreak`；parser `parse_try_block`；desugar 各 walker 补递归；typecheck：`context.rs::try_loop_bases`、`check_expr/ctrl.rs` 的 `check_try_block`（desugar 为 `loop { ..; break <Kind>::<OkVariant>(<tail>) }`）+ `TryBlock`/`TryBreak` 分支 + 裸 `break`/`continue` 越界拦截（`TC016d`）+ `check_question` 残留分流（`try` 内 `?` 改 `Return`→`TryBreak`，错误类型仍取 `current_return_type` 的 `E`，`From` 转换原样复用，**无需类型推断**）；`error.rs` 新增 `BreakOutsideLoop`；`rlyeh-driver`/`rlyeh-fmt`/`rlyeh-check` 同步新节点。验收 `tests/run-pass/eh_try_block.{rl,out}`（6 行），全量 **346/346**。**限制**：`try` 须位于返回 `Result`/`Option` 的函数内（否则 `TC025`）；不支持裸 `break`/`continue`。EH-6（M1/M2/M3）至此全部收口 |
