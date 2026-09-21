# EH-7 错误上下文 / 背链附加（可选）

> **级别**：P3 · **风险**：🟢 低 · **状态**：✅ 完成（2026-09-21，`DynError` 侧形态） · **归属**：0.2.0-AA
> **索引**：[`../rfc/error-handling.md`](../rfc/error-handling.md) §4.7 · **关联**：`eh-2-error-source.md`、`eh-4-dyn-error.md`

## 目标
提供 `context()`/`with_context()` 式组合子，在错误传播路径上附加语义上下文（如「读取配置失败：{path}」），形成可读背链。

## 现状（2026-09-21 落地）
- 无错误上下文附加机制（RFC §2 表中 ❌）。
- 依赖 `source()` 根因链（eh-2）与 `DynError`（eh-4）落地。

## 风险分解与落地
- **M1（低）`Result<T,E>::context(msg)` 包装为带上下文的 `DynError`** ✅
  - `Result<T, DynError>::context(msg: String) -> Result<T, DynError>`：`Err` 直通 `DynError::context(msg)`；`Ok` 原样返回。
  - `DynError::context(self, ctx: String) -> DynError`：把**当前** `DynError`（含既有上下文与消息）`Box::new` 装箱 → `&*b` 取引用 → 上转 `dyn Error` → 以新上下文重新包装。逐层调用即**累积**为 `outer: inner: root` 的可读背链（`source()` 仍直达根因）。
  - `DynError` 新增 `context: String` 字段；`message()` 在非空时拼为 `"<ctx>: <inner message>"`（空串 → 内层消息，保持既有行为）。
  - **位置约束**：`impl<T> Result<T, DynError>` 必须在**根单元**（`rlyeh-std/rlyeh/module.rl`）——`collect_impl` 以「模块前缀 + impl 类型名」构造 self 类型，写在子模块会得到 `io::error::Result`，与根单元的 `Result` 不匹配 → 方法永远找不到。
- **M2（低）`with_context(fn)` 惰性求值** ✅
  - `Result<T, DynError>::with_context(f: fn() -> String)`：仅在 `Err` 分支调用 `f()`。
  - 受益于 EH-3 的 `||` 零参闭包修复（`|| String::from("lazy ctx")` 可直接书写）；受 H2 限制，函数值**不可捕获**外部变量。

## 实现踩坑（已修）
- **协议 impl 的 `message` 是 vtable 分派目标**：`Box<dyn Error>` 虚调用走 `impl DynError: Error::message`，初版只更新了固有 `impl DynError::message`，致经 `dyn` 视图读取时把 `a: b: root` 退化为 `b: root`。二者**必须保持同一上下文语义**。

## 未落地
- 跨具体错误类型（`E` 为任意实现 `Error` 的类型）的 `context`：需泛型 `E: Error` 约束 + 泛型装箱上转；当前仅有各具体类型的 `into_dyn()` 构造器（`IoError::into_dyn` 等）。
- `Context` protocol 形态（RFC §4.7 提及）待办。
- 结构化上下文（文件路径 / 行号字段）未做，当前为字符串消息前缀。

## 受影响组件
`rlyeh-std`（`io/error.rl`：`DynError.context` 字段 + `context` 方法 + `Error` impl 同语义；`rlyeh/module.rl`：`impl<T> Result<T, DynError>`）。

## 验证
- `tests/run-pass/eh_context.rl`（+ `.out`，7 行）：
  - 单层 `context` → `read config: entity not found`；
  - 两层（`startup` → `read config` → root）→ `startup: read config: entity not found`，且 `source().is_none()`（`source()` 未被上下文层级污染，仍直达根因）；
  - `?` 传播保持已附加上下文（`read_cfg().is_err()` 与 `read config: entity not found`）；
  - `with_context(|| String::from("lazy ctx"))` → `lazy ctx: entity not found`；
  - `Ok` 直通（`okv.context("unused").unwrap() == 5`）。
- 全量 `rlyeh test tests`：**344/344 通过**（本轮新增 1 例）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | 评审通过后建叶子 |
| 2026-09-21 | **M1+M2 落地**：`DynError` 增 `context: String` 字段与 `DynError::context`（装箱当前错误 → 上转 → 重新包装，逐层累积为 `outer: inner: root`）；根单元新增 `impl<T> Result<T, DynError>` 的 `context` / `with_context`。修复「协议 impl `message` 与固有方法语义不一致导致 dyn 视图丢上下文」与「`impl Result<..>` 写在子模块致 self 类型名前缀错误」两处踩坑。新增 `tests/run-pass/eh_context.{rl,out}`（7 行），全量 344/344。**阶段 AA（eh-1…eh-8）至此全部收口**（仅 eh-6 的 `try` 块登记为待办） |
