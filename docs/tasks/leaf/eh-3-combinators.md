# EH-3 `Option`/`Result` 组合子补全

> **级别**：P3 · **风险**：🟢 低 · **状态**：✅ 完成（2026-09-21） · **归属**：0.2.0-AA
> **索引**：[`../rfc/error-handling.md`](../rfc/error-handling.md) §4.3 · **关联**：`t1a-vec-api.md`（API 风格参考）

## 目标
补齐常用组合子，消除样板、提升表达力。

## 现状（2026-09-21 实测校正）
- `Option`（`rlyeh-std/rlyeh/core/module.rl`，非叶子早期所述 `core/option.rl`）：实际已有 `is_some`/`is_none`/`unwrap`/`unwrap_or`/`expect`；**新增** `ok_or`/`ok_or_else`/`map`/`and_then`/`or_else`/`filter`/`flatten`/`transpose`/`copied`/`cloned` ✅。
- `Result`（同文件）：实际已有 `is_ok`/`is_err`/`unwrap`/`unwrap_or`/`expect`；**新增** `ok`/`err`/`expect_err`/`map`/`map_err`/`and_then`/`or_else`/`unwrap_or_else`/`flatten`/`transpose`/`copied`/`cloned` ✅。
- 注：叶子初稿「现状」所列 `map`/`and_then`/`propagate` **并不存在**（std 中从未实现），已按实测更正；`propagate` 与 `?`（K1 ✅）语义重复，**决定不提供**。

## 完成清单（全部 ✅ 2026-09-21）
- **非闭包**：`Option::ok_or`、`Result::ok`/`err`/`expect_err`。
- **接收函数值**：`Option::map`/`and_then`、`Result::map`/`map_err`/`and_then`（形参 `fn(..) -> ..`，方法泛型由实参 fn 类型反推）。
- **嵌套泛型**：`Option::flatten`/`transpose`、`Result::flatten`/`transpose`（依赖嵌套 self 类型修复）。
- **引用侧**：`Option::copied`/`cloned`、`Result::copied`/`cloned`（`impl<T> Option<&T>` / `impl<T, E> Result<&T, E>`；`cloned` 经 `where T: Clone`）。
- **惰性 / 零参闭包**：`Option::unwrap_or_else`/`or_else`/`ok_or_else`、`Result::unwrap_or_else`/`or_else`。
- **谓词**：`Option::filter`（形参无方法级泛型）。

## 已修复的编译器缺口（4 处，2026-09-21）
1. ~~typecheck：`fn(..)` 形参中的方法级泛型推断~~ **✅ 已修复**——`crates/rlyeh-typecheck/src/check_expr/method.rs` 的 `check_method_call` 候选循环此前对 fn 形参用 `!matches!(pty, Type::Fn(_))` **一律跳过** unify，致 `fn(T) -> U` 中的 U 无法绑定（调用点报 `undefined type U`，turbofish 亦走不通）。改为：含泛型的 fn 形参在**实参同为 fn 类型**时并入 `unify`（无注解闭包实参仍跳过，避免把泛型绑成 `Infer`）。解锁 `Option::map(inc)` 等**函数值调用**。
2. ~~parser + typecheck：嵌套泛型 self 类型 `impl<T> Option<Option<T>>`~~ **✅ 已修复**：
   - ① **parser**（`crates/rlyeh-parser/src/item.rs`）：impl 的泛型实参此前「消费并丢弃」（`skip_type_generic_args`），且关闭层时未消费 `>>` 拆层留下的 `pending_gt`（`impl<T> Opt<Opt<T>>` 报 `expected '>', found '{'`）。现改为 `parse_type_generic_args` **保留实参**（新增 `AstImplBlock.self_type_args`）并正确消费 pending `>`。
   - ② **typecheck**（`crates/rlyeh-typecheck/src/check_item/collect.rs`）：`ImplDef.self_type` 此前一律由 impl 泛型参数重建（`impl<T> Opt<Opt<T>>` 被压平为 `Opt<T>`，内层实参丢失 → 方法返回类型被实例化为 `Opt<Opt<i64>>` 而非 `Opt<i64>`）；现**优先解析 `self_type_args`**，无实参时回退旧逻辑（`impl Foo` / `impl<T> W` 行为不变）。
   - 解锁 `Option::flatten`/`transpose`、`Result::flatten`/`transpose`。
3. ~~parser：`|| expr` 零参闭包缺表达式前缀分派~~ **✅ 已修复**——`crates/rlyeh-parser/src/expr/primary.rs` 前缀分派此前只把 `Token::BitOr` 交给 `parse_closure`，`Token::OrOr`（`||` 是独立 token）落到 `unexpected("expression")`，`o.unwrap_or_else(|| 7)` 报 `unexpected token: found OrOr`。补 `Some(Token::OrOr) => self.parse_closure(CaptureMode::Borrow)`（前缀位置不存在二元 `||`，其左操作数必须先有表达式，故无歧义）。底层 `parse_closure` 早已支持 `||` 空参列表，仅入口缺失。
4. ~~typecheck：闭包字面量实参对方法泛型的反推~~ **✅ 已修复**（两处配合）：
   - ① `check_expr/closure.rs::check_closure_expected`：期望返回类型**仍含未定泛型**（`ok_or_else(err: fn() -> E)` 的 E、`Result::or_else` 的 F）时，无法据期望定型闭包体，且 `body_ty.compatible_with(Type::Generic(..))` 恒为 false（`compatible_with` 不识别 `Generic`）。现在识别该情形（新增 `generic::contains_any_generic`），**以闭包体推断类型作为匿名函数返回类型**并跳过严格校验，返回的 `Type::Fn` 亦携带体类型。
   - ② `check_expr/method.rs::check_method_call` 实参循环：闭包实参检查后，若 `pty` 仍含未定泛型（impl / 方法级参数名集合）则 `unify(pty, 闭包 fn 类型)` 回填（如 `fn() -> E` × `fn() -> i64` ⇒ `E = i64`）。
   - 效果：`|x| ..` 闭包字面量在**形参泛型可由接收者 / 其它实参反推**时（`Option::map(|x| x + 1)`、`Result::map_err(|e| ..)`）直接可用；泛型仅在闭包返回位置时由体类型回填（`ok_or_else(|| String::from("x"))`、`or_else(|e| Result::Ok(..))`）。

## 仍受限（非本叶子范围）
- **闭包捕获外部变量**：`check_closure_expected` 对捕获报 `TC025 unsupported`（捕获闭包属 H3 规划）。故 `ok_or_else(|| msg)`（捕获 `msg`）仍不可用；可改用 `ok_or(msg)`。
- 方法泛型若**只出现在闭包形参位置**且接收者 / 其它实参无法反推，仍无法定型，请传具名函数 / `fn` 值。
- 同一函数内同名绑定不得跨类型复用（typecheck 变量环境按名索引、无作用域隔离；LIR 亦按名记录类型并直接报冲突）——`docs/std-lib.md` §12。

## 受影响组件
`rlyeh-std`（`rlyeh/core/module.rl` 的 `impl<T> Option<T>` / `impl<T, E> Result<T, E>` / 嵌套泛型 impl / `Option<&T>` / `Result<&T, E>`）；`rlyeh-parser`（`expr/primary.rs`、`item.rs`）；`rlyeh-typecheck`（`check_expr/method.rs`、`check_expr/closure.rs`、`check_expr/generic.rs`、`check_item/collect.rs`）。

## 验证
- `tests/run-pass/eh_combinators.rl`（+ `.out`，49 行）：第一批 `ok_or`（Some→Ok、None→Err、E=i64/E=String）、`ok`/`err`、`expect_err`；第二批 `map`/`and_then`/`map_err`（函数值实参，含 `map→and_then` 链）；第三批 `Option::flatten`、`Result::flatten`、`Option<Result>::transpose`、`Result<Option>::transpose`；第四批 `copied`/`cloned`（含 None/Err 分支）；第五批 `unwrap_or_else`/`or_else`/`ok_or_else`（`||` 零参闭包 + 闭包体回填泛型）、闭包字面量直用（`map(|x| ..)`、`map(|x| x * 2)`）、`filter`（闭包字面量与 fn 值）。与 Rust 同名语义对拍 ✅。
- 全量 `rlyeh test tests`：**340/340 通过**。
- `cargo test --workspace`：仅 `rlyeh-driver/tests/reference_test.rs` 4 例失败，经 `git stash` 对比确认为**既有失败**（借用检查相关，与本次无关）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | 评审通过后建叶子 |
| 2026-09-21 | 落地非闭包组合子 `Option::ok_or` + `Result::ok`/`err`/`expect_err` + `eh_combinators.rl{.out}`；实测校正叶子「现状」（原列 `map`/`and_then`/`propagate` 实不存在）；登记闭包型（方法泛型推断）与嵌套泛型（`Option<Option<T>>` 解析）两类编译器缺口，状态置 🟠 部分完成 |
| 2026-09-21（二次） | **修复 typecheck 缺口**（`method.rs` 候选循环对含泛型的 fn 形参并入 unify）→ 落地函数值型组合子 `Option::map`/`and_then`、`Result::map`/`map_err`/`and_then`；用例扩至 19 行输出 |
| 2026-09-21（三次） | **修复 parser + typecheck 嵌套 self 类型缺口**（`item.rs` 保留 impl 泛型实参 + 消费 `pending_gt`；`collect.rs` 优先用 `self_type_args` 构建 self 类型）→ 落地嵌套泛型组合子 `Option::flatten`/`transpose`、`Result::flatten`/`transpose`；用例扩至 28 行输出 |
| 2026-09-21（四次） | 落地**引用侧组合子** `Option::copied`/`cloned`、`Result::copied`/`cloned`（`impl<T> Option<&T>` / `impl<T, E> Result<&T, E>`；`cloned` 经 `where T: Clone`）；用例扩至 33 行输出。EH-3 仅剩「闭包字面量实参反推 / 0 参 `fn` 实参」一类（`or_else`/`unwrap_or_else`/`ok_or_else`） |
| 2026-09-21（五次，本项收口） | **修复 parser `||` 零参闭包前缀分派** + **typecheck 闭包字面量反推方法泛型**（`contains_any_generic` + 闭包体回填 + `method.rs` 实参循环 unify）→ 落地惰性 / 零参闭包组合子 `Option::unwrap_or_else`/`or_else`/`ok_or_else`、`Result::unwrap_or_else`/`or_else` 与谓词 `Option::filter`；闭包字面量旧限制放宽（`map(|x| ..)` 可用）；用例扩至 49 行输出；**状态置 ✅ 完成**。同步更新 `std-lib.md` §2.1/§2.2（补全 API 清单、移除不存在的 `Option::from`/`Result::propagate`）与 RFC `error-handling.md` §4.3 |
