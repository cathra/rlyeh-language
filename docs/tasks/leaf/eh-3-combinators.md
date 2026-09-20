# EH-3 `Option`/`Result` 组合子补全

> **级别**：P3 · **风险**：🟢 低 · **状态**：🟠 部分完成（非闭包组合子 ✅ 2026-09-21；闭包型/嵌套泛型组合子受阻，见下） · **归属**：0.2.0-AA
> **索引**：[`../rfc/error-handling.md`](../rfc/error-handling.md) §4.3 · **关联**：`t1a-vec-api.md`（API 风格参考）

## 目标
补齐常用组合子，消除样板、提升表达力。

## 现状（2026-09-21 实测校正）
- `Option`（`rlyeh-std/rlyeh/core/module.rl`，非叶子早期所述 `core/option.rl`）：实际已有 `is_some`/`is_none`/`unwrap`/`unwrap_or`/`expect`；**新增** `ok_or` ✅。
- `Result`（同文件）：实际已有 `is_ok`/`is_err`/`unwrap`/`unwrap_or`/`expect`；**新增** `ok`/`err`/`expect_err` ✅。
- 注：叶子初稿「现状」所列 `map`/`and_then`/`propagate`/`map_err` **并不存在**（std 中从未实现），已按实测更正。

## 待补清单与受阻项
- 已完成（本次）：`Option::ok_or`、`Result::ok`/`err`/`expect_err`。
- **受阻（需编译器能力，非 std 层可解）**：
  - **接收函数值**的组合子 `Option::map`/`and_then`/`or_else`/`unwrap_or_else`/`ok_or_else`、`Result::map`/`map_err`/`and_then`/`or_else`/`unwrap_or_else`——受阻于 typecheck：**方法级泛型参数仅出现在 `fn(..)` 形参类型中时无法推断**（实测 `fn apply<U>(&self, f: fn(i64) -> U) -> U` 调用点报 `undefined type U`；即使 turbofish 亦走不通）。
  - **嵌套泛型**组合子 `Option::flatten`/`transpose`、`Result::flatten`/`transpose`/`copied`/`cloned`——受阻于 parser：**`impl<T> Option<Option<T>>`（嵌套泛型 self 类型）不被解析支持**（`>>` 被误读为移位，语法错误）。

## 受影响的编译器缺口（建议转编译器专项）
1. typecheck：`fn(..)` 形参中的方法级泛型推断（`check_method_call`/`unify` 对 `Type::Fn` 的泛型绑定）。
2. parser：嵌套泛型实参 `>>` 的消歧（`Option<Option<T>>`）与嵌套泛型 self 类型的 impl 解析。

## 受影响组件
`rlyeh-std`（`rlyeh/core/module.rl` 的 `impl<T> Option<T>` / `impl<T, E> Result<T, E>`）。

## 验证
- `tests/run-pass/eh_combinators.rl`（+ `.out`）：`ok_or`（Some→Ok、None→Err、E=i64/E=String）、`ok`/`err`（Ok/Err 双分支）、`expect_err`，输出 `7/42/missing/1/1/1/boom/9/5/-1`，与 Rust 同名语义对拍 ✅。
- 注意：同一函数内同名绑定不得跨类型复用（typecheck 变量环境按名索引、无作用域隔离；LIR 亦按名记录类型），用例已用唯一变量名规避。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | 评审通过后建叶子 |
| 2026-09-21 | 落地非闭包组合子 `Option::ok_or` + `Result::ok`/`err`/`expect_err` + `eh_combinators.rl{.out}`；实测校正叶子「现状」（原列 `map`/`and_then`/`propagate` 实不存在）；登记闭包型（方法泛型推断）与嵌套泛型（`Option<Option<T>>` 解析）两类编译器缺口，状态置 🟠 部分完成 |
