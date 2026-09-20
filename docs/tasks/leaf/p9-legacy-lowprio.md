# P9 遗留低优项收口

> **所属专项**：[专项开发计划](../专项开发计划.md)（P9）
> **来源缺陷**：[`leaf/legacy-misc.md`](legacy-misc.md)（#1/#3/#4/#5 遗留限制）
> **状态**：✅ 大部分完成（2026-08-29：P9a `rlyeh new` lib 模板导出函数 ✅、P9b `String::from` 多层直链 ✅、P9c `&dyn Protocol` 参数/返回值 ✅；P9d 空数组字面量 ⏸️ 不实施——见下）
> **风险**：低（各子项独立）
> **前置能力**：依各子任务

## 目标

收口 legacy-misc 中登记的低优先级遗留项：`rlyeh new` 增强、`String::from` 追踪扩展、dyn Protocol 参数/返回值、空数组字面量/模式。

## 子任务

### P9a `rlyeh new` 增强（LM#1，低风险）

- **现状**：`main.rs` `run_new` 仅识别 `--lib`；任何 `-` 开头非 `--lib` 参数报错（`main.rs:672`）。lib 模板空壳（无导出函数）；`version`/`edition` 硬编码；`verbose=false` 固定。
- **方案**：`--target`/`--toolchain` 选项 + lib 模板补导出函数（`pub fn hello()`）。
- **涉及**：`crates/rlyeh-driver/src/main.rs` + `dagon/src/commands.rs`
- **验收**：`rlyeh new` 支持新选项；lib 模板含导出函数。
- ✅ **已完成（lib 模板部分）**：`dagon/src/commands.rs:93-97` 的 lib 模板从空壳注释 `// {name} 库入口`
  改为含 `pub fn hello() -> String { String::from("Hello from {name}!") }` 的导出函数。
  验证：`rlyeh new demolib --lib` 生成 `src/lib.rl` 含 `pub fn hello()`；
  `rlyeh build src/lib.rl` 通过 typecheck/codegen（链接阶段缺 `_main` 为库项目预期行为）。
- ⏸️ **`--target`/`--toolchain` 不实施**：`Manifest`（`dagon/src/manifest.rs`）无对应字段，
  且目标平台/工具链选择由 driver **构建期**选项（`rlyeh build --target`、交叉编译工具链探测）
  处理，在 `new` 时固化会与构建期覆盖冲突。

### P9b `String::from` 追踪扩展（LM#3，低风险）

- **现状**：`local_inits`（`context.rs:229-253`）只追踪 `let` 绑定、只认直链字面量（一层 `construct.rs:918-929`）、fn 边界不穿透。
- **方案**：多层直链（变量=变量=字面量）。
- **涉及**：`check_expr/construct.rs` + `context.rs`
- **验收**：`let a="x"; let b=a; String::from(b)` 可解析。
- ✅ **已完成**：`construct.rs:923-930` 的 `check_string_from` 从「单层 `lookup_local_init` +
  仅认 `StringLiteral`」改为**循环追踪**——init 为 `Variable` 时继续查其 init，深度上限 8
  （防自引用/长链开销；fn 边界由 `lookup_local_init` 天然不穿透）。
  验证：`let a="x"; let b=a; let c=b; String::from(c)` → `x`（三层直链）；
  一层 `let d="y"; String::from(d)` → `y`（无回归）。

### P9c dyn Protocol 参数/返回值（LM#4，连 P4）

- **现状**：`fn_sig.rs:74-94` H4 MVP 限制——`dyn Protocol` 为 2 槽胖指针，仅支持 `let d: dyn Protocol = &obj;` 局部变量主路径，**暂不支持作函数/方法参数与返回值**。
- **方案**：连 P4（dyn 上转型）一并解决，胖指针作参数/返回值。
- **涉及**：`check_item/fn_sig.rs`
- **验收**：`fn f(e: &dyn Error) -> &dyn Error` 可写。
- ✅ **已完成（引用形式 `&dyn Protocol`）**：P4 的 `&dyn` 上转型 + `method.rs` 的 `Ref(Dyn)`
  虚调用扩展后，**`&dyn Protocol` 作参数与返回值均已可用**（H4 的检查仅针对值形式 `Type::Dyn`，
  `Type::Ref(Dyn)` 不受限）。
  验证：`fn describe(e: &dyn Error) -> String` + `describe(d)` → `my error`；
  `fn pass(e: &dyn Error) -> &dyn Error`（参数 + 返回值透传）→ `d2.message()` = `my error`。
- ⏸️ **值形式 `dyn Protocol` 参数/返回值仍保留 H4 限制**：`fn f(e: dyn Error)` 报
  「`dyn Error` 作为函数/方法参数（H4 MVP 仅支持局部变量）」——2 槽胖指针需 LIR 参数/返回
  槽支持（当前 LIR 为标量槽），属独立能力，超出 P9 低优范围（引用形式已覆盖主要用例）。

### P9d 空数组字面量/模式（LM#5，低风险）

- **现状**：`index_enum.rs` 空数组字面量 `[]` MVP 不支持；元组/结构体模式、范围模式 MVP 不支持。
- **方案**：`[]` 字面量支持（可 `Vec::new()` 替代，低优）。
- **涉及**：`check_expr/index_enum.rs`
- **验收**：`let v: Vec<i64> = [];` 可用。
- ⏸️ **不实施**（记录决策）：`index_enum.rs:143-148` 的 `check_array_lit` 对空数组报
  「无法推断元素类型」。实测**即便支持空数组字面量，`let v: Vec<i64> = []` 仍不成立**——
  数组字面量的类型是 `[i64; N]`（`Type::Array`），与 `Vec<i64>` 是不同类型且无自动转换
  （实测 `let v: Vec<i64> = [1,2]` 报 `expected Vec<i64>, found [i64; 2]`）。
  故空数组仅对**零长数组** `[T; 0]` 有意义（实用性极低）；
  Vec 空构造由 `Vec::new()` / `Vec::<T>::new()`（P7b turbofish）完全覆盖。
  若要支持 Rust 风格 `let v: Vec<i64> = []`，需先引入 **Array→Vec 转换**（独立能力，超出 P9 低优范围）。

## 执行步骤

1. P9a：`rlyeh new` 选项 + lib 模板增强
2. P9b：`String::from` 多层直链追踪
3. P9c：dyn Protocol 参数/返回值（连 P4）
4. P9d：空数组字面量支持

## 验收标准

- 各子项独立验证
- 回归全绿

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-28 | 由专项开发计划 P9 生成叶子文档（拆分 P9a/b/c/d） |
| 2026-08-29 | ✅ P9a：`dagon/src/commands.rs` lib 模板从空壳注释改为含 `pub fn hello() -> String`（导出函数起点）；`--target`/`--toolchain` 不实施（Manifest 无字段 + 构建期选项已覆盖）。✅ P9b：`construct.rs check_string_from` 的 `local_inits` 追踪改为循环多层（深度上限 8），`let a="x"; let b=a; let c=b; String::from(c)` 可用。✅ P9c：`&dyn Protocol` 作参数/返回值可用（`fn pass(e: &dyn Error) -> &dyn Error`）；值形式 `dyn Protocol` 仍保留 H4 限制（需 LIR 胖指针槽）。⏸️ P9d 不实施：数组字面量为 `[T; N]`，与 `Vec<T>` 无自动转换（实测 `[1,2]` 赋给 `Vec<i64>` 亦报错），空数组仅对 `[T;0]` 有意义；Vec 空构造由 `Vec::new()`/`Vec::<T>::new()` 覆盖。cargo test（含 suite_test）全绿 |
