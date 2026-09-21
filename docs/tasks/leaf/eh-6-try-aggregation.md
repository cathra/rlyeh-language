# EH-6 错误聚合 / `try` 块

> **级别**：P3 · **风险**：🟠 中 · **状态**：🟢 部分完成（M2 / M3 ✅ 2026-09-21；M1 `try` 块 ⏳ 待办，**前置 `loop { break <value> }` ✅** 2026-09-21） · **归属**：0.2.0-AA
> **索引**：[`../rfc/error-handling.md`](../rfc/error-handling.md) §4.6 · **关联**：`eh-4-dyn-error.md`

## 目标
- `try` 块：块内 `?` 不立即返回，而是聚合首个错误（或 `Vec` 累积全部错误）后统一处理。
- 迭代器错误累积：`Vec<Result<T,E>>` → `Result<Vec<T>, E>`（类 `collect`/`try_collect`）。

## 现状（2026-09-21 落地）
- `?` 仅支持**早返回**（`check_question` 合成 `match … { Err(e) => return Err(e) }`，硬编码到函数边界）；无块级聚合机制。

## 风险分解与落地
- **M1（中）`try` 块 desugar（`Result<_, E>` 收集到隐藏累加器）** ⏳ **未落地**（前置 ✅ 2026-09-21）
  - **前置已落地：`loop { break <value> }` 循环值** ✅（叶子「建议实施顺序」第 1 条）——`break e` 的值此前在 typecheck（`Loop` 恒 `Never`）与 MIR（显式丢弃 + 出口块恒 `Unit`）**两处**被丢，现补齐 `TypeContext::loop_break_types` 与 `LoopCtx::break_value`，`let x = loop { break 5 };` 得 `x = 5`。验收 `tests/run-pass/loop_break_value.{rl,out}`（10 行）。**原阻塞 #4（`break <value>` 通路未验证）已消除**。
  - **剩余阻塞与已探明结论**：
    1. `try` **不是关键字**（lexer 无 `Token::Try`）——已核实 std / tests 中**未**将 `try` 用作标识符，新增关键字风险低。
    2. `?` 的失败臂在 `check_question`（`check_expr/index_enum.rs`）中**合成 AST `return`**，并用 `ctx.current_return_type` 决定 `From` 转换目标。**残留上下文只需一个深度计数**：`try` 块内的 `?` 把合成体由 `Return(..)` 改为 `Break(<Err/None>(..))` 即可——错误类型仍是**外层函数的错误类型**（`ctx.current_return_type` 不变），故无需类型推断、无鸡生蛋问题（`Err` 的 `From` 自动转换逻辑原样复用）。
    3. **desugar 形态已探针验证**：`try { body }` → `loop { <body stmts>; break <Kind>::<OkVariant>(<tail>) }`（`Kind` = `Result`/`Option`，取自 `current_return_type`）。关键机制均实测可用：`let r: Result<i64, String> = loop { break Result::Ok(3); }` ✅；多臂 `break`（`Err` 在 `Ok` 前）✅；裸 `Result::Ok(3)`（`E` 未定）在本编译器**宽松可接受**（`let x = Result::Ok(3); x.is_ok()` ✅），故无需为 `E` 引入注解包装。
    4. **`try` 块类型须显式构造**：`?` 的残留 `break` 的 `Result<?, E>` 的 Ok 位未定，若它**先**登记会污染循环类型（首登记为准）。方案：为 `?` 残留引入独立 AST 节点（拟 `ExprKind::TryBreak`），其 desugar 仍为 HIR `Break(Some(..))`，但**不登记**循环 break 值类型；于是仅 `break Ok(tail)` 登记 → 循环类型 = `Result<T_tail, ?>`，`T_tail` 具体，`check_try_block` 再以 `current_return_type` 的 `E` 显式组装为 `Result<T_tail, E>` 返回。
    5. **隐式循环的 `break` / `continue` 越界风险须拦截**：`try` 体若含裸 `break` / `continue`，会绑定到**隐式循环**（`continue` 将重启 try 体，静默错误）。方案：进 `try` 块时记录 `loop_break_types.len()` 基准，`continue` 与裸 `break` 在该深度上出现即报错（嵌套的真实循环深度更深，不受影响）。
  - 受影响组件（待改）：`rlyeh-lexer`（`Token::Try`）、`rlyeh-ast`（`ExprKind::TryBlock` / `ExprKind::TryBreak`）、`rlyeh-parser`、`rlyeh-desugar`（各 walker 补新节点）、`rlyeh-typecheck`（`try_depth` 计数 + `check_try_block` + `check_question` 分支 + 越界拦截）。
- **M2（中）迭代器 `try_collect` / `Vec` 错误累积** ✅
  - `rlyeh-std/rlyeh/core/module.rl` 新增 `impl<T, E> Vec<Result<T, E>>`：
    - `try_collect(self) -> Result<Vec<T>, E>`：**首错短路**（对齐 Rust `collect::<Result<Vec<T>, E>>()`）；
    - `collect_errors(self) -> Vec<E>`：忽略 `Ok`，按原序累积**全部**错误。
  - 依赖嵌套泛型 self 类型 `impl<T, E> Vec<Result<T, E>>`（EH-3 期间修复）。
  - `try_for_each` 未落地（需接收 `Result<T, E>` 的函数值，价值低于前两者）。
- **M3（低）与 `DynError`（eh-4）组合：聚合为 `Vec<DynError>`** ✅
  - `Vec<Result<T, DynError>>::try_collect()` / `::collect_errors()` 直接可用（`DynError` 为具体 struct），异质错误统一聚合后逐条 `message()`。

## 受影响组件
`rlyeh-std`（`core/module.rl` 新增 `impl<T, E> Vec<Result<T, E>>`）；**前置已落地**：`rlyeh-typecheck`（`context.rs::loop_break_types`、`check_expr/ctrl.rs` 的 `While` / `Loop` / `Break` 分支）、`rlyeh-mir`（`lower.rs` 的 `LoopCtx::break_value` / `HirExprKind::Break` / `lower_loop`）、`tests/run-pass/loop_break_value.{rl,out}`；（M1 待办将涉及 `rlyeh-lexer` / `rlyeh-parser` / `rlyeh-ast` / `rlyeh-typecheck` / `rlyeh-desugar`）。

## 验证
- `tests/run-pass/eh_aggregate_errors.rl`（+ `.out`，12 行）：
  - `try_collect` 全 Ok（`len = 3`）/ 首错短路（`is_err = 1`、错误值 `bad`）；
  - `collect_errors` 多错聚合（`len = 2`，逐条 `e1` / `e2`）；
  - 空集合（`try_collect().unwrap().len() = 0`、`collect_errors().len() = 0`）；
  - M3：`Vec<Result<i64, DynError>>` 的 `try_collect().is_err()` 与 `collect_errors()` 逐条 `message()`（`entity not found` / `io error`）。
- 全量 `rlyeh test tests`：**343/343 通过**（本轮新增 1 例）。
- 前置（`break <value>`）：`tests/run-pass/loop_break_value.{rl,out}`（10 行：单值 / 多臂汇合 / `break;` → `()` / 嵌套 / 查找惯用法 / 字符串与浮点值 / `while`·`for` 不回退），全量 **345/345 通过**。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | 评审通过后建叶子 |
| 2026-09-21 | **M2+M3 落地**：`impl<T, E> Vec<Result<T, E>>` 的 `try_collect`（首错短路）/ `collect_errors`（全错聚合），与 `DynError` 组合验证；新增 `tests/run-pass/eh_aggregate_errors.{rl,out}`（12 行），全量 343/343。**M1 `try` 块登记为 ⏳ 待办**，附 5 条阻塞分析（`try` 非关键字 / `?` 硬编码函数级 `return` 需残差上下文 / `break <value>` codegen 未验证 / 隐藏函数方案受阻于闭包捕获不支持 / 建议先补 `break <value>` 通路） |
| 2026-09-21 | **M1 前置落地：`loop { break <value> }` 循环值**——实测 `let x = loop { break 5 };` 中 `x` 无值（`println(x)` 输出空行、无报错）：`break e` 的值在 **typecheck**（`ExprKind::Loop` 恒 `Type::Never`）与 **MIR**（`lower.rs` 注释「MVP 阶段丢弃」+ `lower_loop` 出口块恒 `Unit`）两处被丢。补齐 `TypeContext::loop_break_types`（循环 break 值类型栈，`while` 亦压栈占位以防误写外层槽）与 `LoopCtx::break_value`（循环体降低前分配承载槽，带值 `break` 写入后跳出，出口块读该槽）。新增 `tests/run-pass/loop_break_value.{rl,out}`（10 行），全量 **345/345**。**阻塞 #4 消除**；其余设计经探针逐一验证并记入上节（残留上下文只需深度计数、desugar 形态、裸 `Result::Ok` 的 `E` 宽松、`?` 残留须用独立 AST 节点、须拦截 `try` 体内裸 `break`/`continue`） |
