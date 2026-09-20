# EH-3 `Option`/`Result` 组合子补全

> **级别**：P3 · **风险**：🟢 低 · **状态**：🟠 部分完成（非闭包 + 函数值型组合子 ✅ 2026-09-21；闭包字面量实参 / 嵌套泛型组合子受阻，见下） · **归属**：0.2.0-AA
> **索引**：[`../rfc/error-handling.md`](../rfc/error-handling.md) §4.3 · **关联**：`t1a-vec-api.md`（API 风格参考）

## 目标
补齐常用组合子，消除样板、提升表达力。

## 现状（2026-09-21 实测校正）
- `Option`（`rlyeh-std/rlyeh/core/module.rl`，非叶子早期所述 `core/option.rl`）：实际已有 `is_some`/`is_none`/`unwrap`/`unwrap_or`/`expect`；**新增** `ok_or` ✅。
- `Result`（同文件）：实际已有 `is_ok`/`is_err`/`unwrap`/`unwrap_or`/`expect`；**新增** `ok`/`err`/`expect_err` ✅。
- 注：叶子初稿「现状」所列 `map`/`and_then`/`propagate`/`map_err` **并不存在**（std 中从未实现），已按实测更正。

## 待补清单与受阻项
- 已完成（2026-09-21）：
  - **非闭包**：`Option::ok_or`、`Result::ok`/`err`/`expect_err`。
  - **接收函数值**：`Option::map`/`and_then`、`Result::map`/`map_err`/`and_then`（形参 `fn(..) -> ..`，方法泛型由实参 fn 类型反推）。
- **仍受阻**：
  - `Option::or_else`/`unwrap_or_else`/`ok_or_else`、`Result::or_else`/`unwrap_or_else`——需 0 参函数值 `fn() -> ..` 或闭包实参；**闭包字面量实参**不能反推方法泛型（闭包体类型无法脱离上下文定型），0 参 fn 类型可行性待评估。
  - **嵌套泛型**：`Option::flatten`/`transpose`、`Result::flatten`/`transpose`/`copied`/`cloned`——受阻于 parser：`impl<T> Option<Option<T>>`（嵌套泛型 self 类型）不被解析支持（`>>` 被误读为移位）。

## 已修复的编译器缺口（2026-09-21）
1. ~~typecheck：`fn(..)` 形参中的方法级泛型推断~~ **✅ 已修复**——`crates/rlyeh-typecheck/src/check_expr/method.rs` 的 `check_method_call` 候选循环此前对 fn 形参用 `!matches!(pty, Type::Fn(_))` **一律跳过** unify，致 `fn(T) -> U` 中的 U 无法绑定（调用点报 `undefined type U`）。改为：含泛型的 fn 形参在**实参同为 fn 类型**时并入 `unify`（无注解闭包实参仍跳过，避免把泛型绑成 `Infer`，交由下方实参循环按预期签名检查）。修复后 `Option::map(inc)` 等**函数值调用**可用；全量 340/340 无回归。

## 仍受阻的编译器缺口
- **parser**：嵌套泛型实参 `>>` 的消歧（`Option<Option<T>>`）与嵌套泛型 self 类型的 impl 解析。
- **typecheck**：闭包字面量实参对方法泛型的反推（需按预期签名先定型闭包体、再回填泛型，`check_closure_expected` 未接 subst）。

## 受影响组件
`rlyeh-std`（`rlyeh/core/module.rl` 的 `impl<T> Option<T>` / `impl<T, E> Result<T, E>`）。

## 验证
- `tests/run-pass/eh_combinators.rl`（+ `.out`，19 行）：第一批 `ok_or`（Some→Ok、None→Err、E=i64/E=String）、`ok`/`err`（Ok/Err 双分支）、`expect_err`；第二批 `map`/`and_then`（Option，Some/None）、`map`/`map_err`/`and_then`（Result，Ok/Err）+ `map→and_then` 链，输出 `7/42/missing/1/1/1/boom/9/6/1/5/1/6/7/zero/4/11/5/-1`，与 Rust 同名语义对拍 ✅。
- 注意：同一函数内同名绑定不得跨类型复用（typecheck 变量环境按名索引、无作用域隔离；LIR 亦按名记录类型），用例已用唯一变量名规避。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | 评审通过后建叶子 |
| 2026-09-21 | 落地非闭包组合子 `Option::ok_or` + `Result::ok`/`err`/`expect_err` + `eh_combinators.rl{.out}`；实测校正叶子「现状」（原列 `map`/`and_then`/`propagate` 实不存在）；登记闭包型（方法泛型推断）与嵌套泛型（`Option<Option<T>>` 解析）两类编译器缺口，状态置 🟠 部分完成 |
| 2026-09-21（二次） | **修复 typecheck 缺口**（`method.rs` 候选循环对含泛型的 fn 形参并入 unify）→ 落地函数值型组合子 `Option::map`/`and_then`、`Result::map`/`map_err`/`and_then`；用例扩至 19 行输出；剩余受阻项收敛为「闭包字面量实参反推」与「嵌套泛型 self 类型解析」 |
