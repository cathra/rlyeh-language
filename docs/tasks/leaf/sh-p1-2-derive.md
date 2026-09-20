# SH-P1-2 protocol derive 宏

> **级别**：P1（阻塞前端自举） · **状态**：🟢 核心落地（struct 三 protocol） · **归属**：0.2.0-C
> **索引**：[`../self-hosting-p1.md`](../self-hosting-p1.md) · **评估**：[`../../self-hosting/feasibility.md`](../../self-hosting/feasibility.md) §4 P1-5 · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.3

## 目标
提供 `#[derive(Debug/Clone/PartialEq)]` 等**编译期 protocol 自动实现**框架，消除 AST/HIR/MIR/LIR 的 `derive(Debug/Clone/PartialEq)`×70+ 样板，使 Rlyeh 侧重写这些 IR 时可表达等价派生。

## 技术细节
- 当前 Rlyeh 0.1.0：「无 protocol derive 宏」（见 `docs/guide/13-references-limits.md`）。已有 `macro_rules!` 声明式宏（I1），本任务扩展为**属性宏 + 编译期 protocol 自动实现**。
- 受影响 Rust 代码（事实依据，来自 `crates/` 核查）：`rlyeh-ast/src/lib.rs` `derive`×33、`rlyeh-hir`×15、`rlyeh-typecheck/types.rs`×10、工作区 `serde`/`clap` 的 `features=["derive"]`。
- 子任务（对应 0.2.0-C）：C1 `#[derive(Debug)]` / C2 `#[derive(Clone)]` / C3 `#[derive(PartialEq)]` / C4 derive 宏框架（用户可扩展）。

## 实现纪要（2026-09-02，核心落地）
- **机制**：derive 展开 **零新增 IR 节点**——`parse_attributes` 已收集 `#[derive(...)]` 名列表；
  `collect_item_decls` 在收集结构体后调 `expand_derives_for_struct`（`crates/rlyeh-typecheck/src/check_item/derive.rs`），
  为每个受支持 protocol 合成一个 `AstImplBlock` 并走既有 `collect_impl`（与手写 `impl` 完全一致），
  方法体在调用点实例化。
- **C2 `Clone`**：`protocol Clone { fn clone(&self) -> Self; }`（`core.rl` 顶层新增）；
  derive 生成逐字段拷贝——标量 / 引用直接拷贝，其余（String / 嵌套 struct / 泛型字段）调 `.clone()`。
- **C3 `PartialEq`**：`protocol PartialEq { fn eq(&self, other: &Self) -> bool; }`（`core.rl` 顶层新增）；
  derive 生成逐字段 `==` 链；并在 `comparison.rs` 将结构体 `==`/`!=` **desugar 为 `a.eq(&b)` / `!a.eq(&b)`**
  （仅当该类型实现了 `PartialEq`——手写或 derive，否则保留「仅 String 支持内容相等比较」错误）。
- **C1 `Debug`**：复用 `fmt` 模块既有 `protocol Debug { fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }`；
  derive 生成 `fmt` 体——标量走 `int_to_string`/`float_to_string`、bool 走字面量、String 走 `write_str`、
  嵌套类型走 `.fmt(f)`；经 `dbg!`（`{:?}` 语义）输出 `Name { f0: <Debug>, ... }`。
- **C4 框架**：`canonical_protocol` 映射受支持名（`Clone`/`PartialEq`/`fmt::Debug`），未知名（如 serde
  `Serialize`/`Deserialize`）宽松忽略（不报错、不生成 impl），与既有行为兼容。

## 受影响组件
`rlyeh-parser`（`parse_attributes` 已有）、`rlyeh-ast`（`AstStructDecl.derive` 已有）、
新增 `rlyeh-typecheck/src/check_item/derive.rs`、`rlyeh-typecheck/src/comparison.rs`（desugar）、
`rlyeh-std/rlyeh/core.rl`（`Clone`/`PartialEq` protocol）、`rlyeh-std/rlyeh/fmt/module.rl`（`Debug` 既有）。

## 验证
- 新增 run-pass：`tests/run-pass/derive_clone.rl`（拷贝 `1/2/7/hi`）、`derive_partialeq.rl`（`1/0/1`）、
  `derive_debug.rl`（`dbg: Point { x: 1, y: 2 }`），均含同名 `.out`。
- 全量 `rlyeh test tests`：**252 用例全过**（含 `x3_deserialize*` serde derive 用例未受影响）。

## 状态
🟢 struct 的 Clone / PartialEq / Debug derive 已落地并全量回归通过；enum derive、泛型 bound 强制校验、用户自定义 derive 宏（C4 扩展点）留待后续。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P1-5 拆出为叶子 |
| 2026-09-02 | struct `#[derive(Clone/PartialEq/Debug)]` 核心落地：derive 展开 + `==`/`!=` desugar + std protocol；全量回归 252/252 |
