# SH-P0-8 `Drop` trait / 析构 / RAII

> **级别**：P0（阻塞运行时重写 + IR 资源清理） · **风险**：🔴 高 · **状态**：✅ 已完成（Q1–Q3；Q4 智能指针接入待办） · **归属**：0.2.0-Q
> **索引**：[`../self-hosting-p0.md`](../self-hosting-p0.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.17

## 目标
支持 `trait Drop { fn drop(&mut self); }` + `impl Drop for T`，作用域结束（拥有所有权变量）自动调用 `drop`，实现 RAII：MutexGuard 自动解锁、arena 自动释放、`Rc`/`Arc` 计数递减、`File` 自动关闭。Rlyeh 0.1.0 完全无析构机制（MVP 靠注入解锁/OS 回收，std-lib.md 多处标注「无 Drop」）。

## 现状
- 语言无 `Drop` trait；codegen 不发射作用域尾清理。
- 受影响 Rust 事实：`sync/module.rl` 目标 `MutexGuard` 作用域尾自动解锁、`rlyeh-std` 多处「进程退出时由 OS 回收（无 Drop）」、`Vec`/`HashMap` 析构。

## 风险分解（→ 中/低危）
- **M1（中）** `trait Drop { fn drop(&mut self); }` 声明 + `impl Drop for T` 检测（与现有 trait/impl 机制复用，需 A 泛型 trait 落地后接）。
- **M2（中）** 作用域尾自动插入 `x.drop()`：仅对**拥有所有权**的栈变量，按声明逆序；临时值与借用变量跳过（借用检查 G1 协同）。
- **M3（中）** 字段级 drop：struct 含拥有字段（如 `Vec` 内部堆指针）在变量 drop 时递归 drop 其拥有字段。
- **M4（中）** 智能指针接入 `Drop`：`Box` 释放堆、`Rc`/`Arc` `strong_count-1`（归零释放）、`Gc` 块外逃逸登记。
- **L1（低）** 差分对拍 Rust 参考，确认 drop 调用时序与作用域一致。

## 受影响组件
`rlyeh-typecheck`（drop 插入点分析）、`rlyeh-codegen`（作用域清理代码发射）、`rlyeh-std`（MutexGuard/Vec/HashMap/File 接 Drop）。

## 验证
- Rlyeh 侧 `MutexGuard` 越界自动解锁、`arena` 越界自动释放（计数器/日志断言）。
- 单元：含拥有 `Vec` 字段的 struct 离开作用域触发递归释放，无泄漏。

## 缺口复核（2026-09-02，动手前）
叶子「现状」称「语言无 `Drop` trait；codegen 不发射作用域尾清理」，复核发现
**四项缺口层次各不相同**，且 Q-M2 的**注入机制其实已存在**：

| 项 | 规划描述 | 复核结果 |
|----|---------|---------|
| **Q-M1** `Drop` trait + impl 识别 | 完全缺失 | 语言层**确为缺口**（全部 `Drop` 命中都在 Rust 侧运行时/测试）；但已有内置 trait 先例（`Any`）可同构实现 |
| **Q-M2** 作用域尾插入 `drop` | 完全缺失 | **机制已存在**——`rlyeh-desugar/src/guard.rs` 已有「块尾注入 + 嵌套块递归」，但**硬编码特判方法名 `lock_guard`/`read_guard`/`write_guard`**（AST 阶段无类型信息），非按类型 / Drop impl → 需泛化而非重写 |
| **Q-M3** 字段级递归 | 缺失 | 确为缺口 |
| **Q-M4** 智能指针接入 | 缺失 | 确为缺口（`Box`/`Rc`/`Arc` 未接 `Drop`）→ **本轮未做**，见「已知限制」 |

另：M1 原注「需 A 泛型 trait 落地后接」。复核认为**非阻塞**——`Drop` 本身是
非泛型 trait，具体类型上的 `impl Drop for Foo` 复用既有 trait/impl 机制即可；
仅 `Vec<T>` 这类泛型类型的 Drop 才依赖 0.2.0-A。

## 实现纪要（2026-09-02）

### Q-M1：`Drop` 作为内置 trait
`is_drop_trait`（`check_expr/misc.rs`，与 `is_any_trait` 同构）：`Drop` 无论是否
显式 `trait Drop` 声明都生效，解析名可带模块前缀。`has_drop_impl(ctx, ty)` 按
「trait 名匹配 + `type_matches` 目标类型」在 `ctx.impl_defs` 中查找。

### Q-M2：块尾自动插入 `x.drop()`
- **声明顺序**：`Scope.vars` 是 `HashMap`（无序），而析构必须按**逆声明序**，
  故 `Scope` 新增 `decl_order: Vec<String>`，由 `insert_variable` 维护；
  新增 `TypeContext::current_scope_decls()` 取本层声明（外层变量随作用域弹出
  自然不可见，不会被误析构）。
- **插入点**：`check_expr/block.rs::check_block_inner` 块尾。析构调用以 AST
  `x.drop()` 经 `check_stmt` 走**常规方法解析**——`&mut self` 借用、泛型单态化、
  trait 分派全部复用既有路径，**零新增 IR 节点**。
- **时序**：块值先求、后析构。有值块把 `final_expr` 存入临时变量，析构后再以
  该临时作为块结果（保证 `let v = { let r = ..; r.id }` 中 `r.id` 先求值）。
  `Never` 结尾（`return`）跳过注入，避免在终结指令后追加不可达语句。
- **零影响存量**：无 `Drop` 实现的类型不产生任何语句——当前 std 无任何
  `Drop` 实现，故本次改动对既有代码完全无行为变化（全量回归佐证）。

### Q-M3：字段级递归（drop glue）
`build_drop_glue`：先 `T::drop()`，再按**逆字段序**递归各字段（与 Rust drop
glue 一致）。仅具名 struct 参与递归（枚举 / 元组 / 内建类型不在 `ctx.structs`
内，自然终止）；泛型字段按实例类型实参 `substitute` 后判定；`MAX_DROP_DEPTH = 4`
防止 `struct A { b: B }` / `struct B { a: A }` 自引用无限展开。

## 验收
- `tests/run-pass/drop_raii.{rl,out}`（22 行输出）：逆声明序 / 嵌套块词法作用域 /
  块值先求后析构 / 函数体局部返回前析构 / **引用绑定跳过** / 带返回值函数时序 /
  逆字段序析构 / 先 `drop()` 再字段 glue / 多层嵌套字段递归。
- 全量 `cargo test --workspace -- --test-threads=1` 128 个测试目标全绿。

## 已知限制
- **未跟踪 move**：被 `return` 移出或转移给其他值的变量仍会被析构（MVP）。
- **函数形参不析构**：形参位于外层 fn 作用域，不在函数体块层。
- **提前退出路径不注入**：`return` / `break` 分支不注入析构（与既有
  `guard.rs` 同一约束）。
- **Q-M4 智能指针未接入**：`Box` / `Rc` / `Arc` 尚未实现 `Drop`（需先确定
  堆释放与引用计数递减的调用约定，且泛型类型的 Drop 依赖 0.2.0-A），故
  `sync::MutexGuard` 仍走 `lock_guard` 方法名特判的既有注入路径。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 复审补遗：从运行时重写 + IR 资源清理依赖中拆出（语言完全缺失） |
| 2026-09-02 | 缺口复核（Q-M2 注入机制已存在但按方法名硬编码；M1 不依赖 0.2.0-A）；实现 Q-M1/Q-M2/Q-M3；run-pass 用例与全量回归；Q-M4 记为待办 |
