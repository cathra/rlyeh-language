# SH-P0-3 `dyn Trait` 含 `Self` 方法 + `Any` 类型擦除

> **级别**：P0（阻塞全栈自举） · **风险**：🔴 高 · **状态**：✅ 已完成（单元验证） · **归属**：0.2.0-G
> **索引**：[`../self-hosting-p0.md`](../self-hosting-p0.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.7

## 目标
支持 `dyn Trait` **调用含 `Self` 签名的方法**，并提供 `std::any::Any` 式的**类型擦除 / `downcast`**，使 actor 消息协议（异构消息信封）可用 Rlyeh 表达。

## 技术细节
- 当前 Rlyeh 0.1.0：「`dyn Trait` 约束：trait 与 impl 均须非泛型；方法签名含 `Self`（关联返回类型 / 参数）不支持经 dyn 调用」（见 `CODEBUDDY.md` §3.8 H4）。且**无类型擦除**机制。
- 受影响 Rust 代码（事实依据）：
  - `rlyeh-actor-runtime/src/actor.rs:79` `pub trait ActorState: Any + Send + Sync + 'static`
  - `rlyeh-actor-runtime/src/runtime.rs:31` `Arc<Mutex<Option<Box<dyn ActorState>>>>`、`:87` `Box<dyn Any + Send>`
  - `rlyeh-actor-runtime/src/envelope.rs:21,23` `Box<dyn Any + Send>` + `Sender<Box<dyn Any + Send>>`
  - 编译器内部已把 `dyn Trait` 建模为「数据指针 + vtable 胖指针 2 槽」（`rlyeh-typecheck/src/types.rs:66`）。
- 需设计：vtable 中 `Self` 方法的签名擦除/恢复、`Any` 类型标识存储与 `downcast` 安全检查。

## 受影响组件
`rlyeh-actor-runtime`（actor 状态 trait / 消息信封 / 类型擦除分发）。

## 验证
- 单元：Rlyeh 侧经 `dyn Trait` 调用含 `Self` 返回的方法；`Any` 装箱 + `downcast` 往返成功。
- 对拍：等价于 actor 消息 `handle_message(&mut self, ...)` 分发。

## 状态
✅ 已完成（单元验证）（**0.2.0 必须项（语言特性）**，对应阶段 G；运行时*重写*属 0.3.0）。

## 实现纪要（2026-09-01 完成）

### G-M1：`dyn Trait` 调含 `Self` 签名的方法
- 落点：`rlyeh-typecheck/src/check_expr/method.rs::devirtualize_dyn_call`。
- 原状：`Self` 提及（`type_mentions_self`）直接 `Ok(None)` 回退，随后 vtable 分支
  （`method.rs:779`）再以 object-unsafe 拒绝 → 含 `Self` 的方法**两条路径均不可用**。
- 方案：dyn 变量绑定源具体类型已知时（`ctx.get_dyn_concrete`），用
  `replace_type_self` 把签名中的 `Self` 替换为具体类型，再检查实参、推导返回类型。
  `fn combine(&self, other: &Self) -> i64` 经 `dyn Combine`（绑定源 `P`）
  收敛为 `fn combine(&self, other: &P) -> i64`，静态分派到具体 impl 方法。
- 语义边界：**真正擦除**具体类型的 `dyn Trait`（如作函数参数传入）仍由 vtable 分支
  拒绝含 `Self` 的方法——与 Rust object safety 一致（`Self` 签名在完全擦除后
  无法在调用点确定）。

### G-M2：`Any` 类型标识存储
- 落点：`rlyeh-typecheck/src/check_expr/misc.rs`（`type_id_of` / `is_any_trait` /
  `coerce_to_any`）、`resolve.rs`（`dyn Any` 免 trait 声明）。
- `dyn Any` 为编译器内置的类型擦除标签：`&T → dyn Any` 不查 trait 声明、不建方法表，
  vtable 仅 3 元槽，**槽 0 存具体类型的 type_id**（`type_id_of` = 类型规范字符串的
  FNV-1a 64 位散列，编译期确定、全程序稳定）。
- `any_type_id(x: dyn Any) -> i64` 读 vtable 槽 0。

### G-M3：`downcast` 安全检查
- `any_downcast_ref::<T>(x: dyn Any) -> Option<&T>` 展开为
  `if type_id(x) == type_id_of(T) { Some(data_ptr as &T) } else { None }`
  （`HirExpr::If` + 与 `check_variant_construct` 一致的 `Option` 槽布局）。
  类型标识相等才产出具 `&T` 的 `Some`——**无未检查转换**，错误类型向下转换
  得到 `None` 而非错误解释的引用。
- 门禁：非 `dyn Any` 实参 / 缺 turbofish 类型参数均显式报错。

### 用例
- `tests/run-pass/dyn_self_return.rl`（G-M1，`&Self` 形参 + 按值返回 `Self` 聚合）
- `tests/run-pass/any_downcast.rl`（G-M2/G-M3，装箱 + 成功/失败 downcast 往返）
- `tests/compile-fail/any-downcast-nonany.rl`、`any-downcast-notype.rl`（门禁）
- `tests/run-pass/agg_return_plain.rl`（普通函数按值返回聚合，回归对照）

### 附带修复：按值返回 `Self` 聚合经 dyn 调用（codegen）
- 原缺陷：`fn make(&self) -> Self` 经 `dyn Trait` 调用报
  `value doesn't match function result type 'ptr'`——声明 `i8*` 而函数体
  `ret {i64,i64}`。
- 根因：`rlyeh-codegen/src/llvm/llvm_ctor.rs` 的「标量聚合按值返回」优化中，
  **被取址函数**（vtable 的 `FnPtr` / 函数指针间接调用）须保持稳定的 `i8*` 返回
  ABI，故被排除在 `ret_by_value` 之外；但原实现在被取址分支直接 `continue`，
  跳过了 4b-iv 的**连带剔除**，破坏了「`f ∈ ret_by_value` ⟺ 全部 `Return ∈ bvs`」
  不变量——返回局部变量仍留在 `bvs` 中，函数体按值返回聚合而声明为 `i8*`。
- 修复：改以 `participates` 标志区分——被取址（及 extern / main）函数**不参与**
  `ret_by_value` 判定，但仍执行 4b-iv 连带剔除（含指针拷贝别名闭包），使 Return
  值移出 `bvs`；函数体随之改用 calloc 堆分配并返回 `i8*`，声明 / 函数体 / 调用点
  三方一致。IR 校验：`define i8* @"Pair::make__Copyable"(i64 %self)` +
  `ret i8*`（经 `calloc`）。
- 代价：被取址函数的按值聚合返回退化为堆分配 + 指针返回（ABI 稳定性优先），
  与普通函数（如 `make_pair`）的 `{i64,i64}` 寄存器返回不同。

### 已知限制
- `Box<dyn Any + Send>` / 多 trait 约束（`+ Send`、auto trait）未实现——
  actor 信封完整形态仍待 0.3.0 运行时重写。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P0-3 拆出为叶子 |
| 2026-09-01 | 修正归属：由「0.3.0+ 长期跟踪」上移为 0.2.0-G（与计划 §1/§3.7 一致）；索引由 feasibility 评估改为 development-plan §3.7 |
| 2026-09-01 | G-M1/G-M2/G-M3 实现完成，单元验证通过（全量 740 用例全绿） |
| 2026-09-02 | 修复按值返回 `Self` 聚合经 dyn 调用的 codegen 缺陷（`llvm_ctor.rs` 被取址函数跳过连带剔除，破坏 `ret_by_value`⟺`Return ∈ bvs` 不变量）；`dyn_self_return.rl` 扩展覆盖两种 `Self` 形式 |
