# SH-P1-4 `Deref` / `DerefMut` 用户类型自动解引用强制

> **级别**：P1（阻塞智能指针 / MutexGuard 表达） · **风险**：🟠 中 · **状态**：🟢 M1+M2 落地（2026-09-04） · **归属**：0.2.0-R
> **索引**：[`../self-hosting-p1.md`](../self-hosting-p1.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.18

## 目标
支持用户定义 `Deref`/`DerefMut` trait + 字段/方法/索引访问时自动插入 `(*x)` 解引用强制（deref coercion），使 `MutexGuard`/`Box<dyn Trait>`/智能指针可透明透传内部字段与方法，无需手写 `(*guard).x`。

## 现状
- MVP 仅**编译器内建**类型（`Box`/`Rc`/`Arc`）自动剥层；用户定义 `Deref` 与自动解引用强制未实现（std-lib.md 目标 API 含 `impl Deref for MutexGuard` 但未落地）。

## 风险分解（→ 中/低危）
- **M1（中）** `trait Deref { type Target; fn deref(&self) -> &Self::Target; }` + `DerefMut`（依赖 A 关联类型 / G `dyn` 落地后接）。✅ 2026-09-04
- **M2（中）** 自动解引用：字段/方法/索引访问失败时，尝试对接收者插入 `x.deref()` 递归（限循环深度，避免无限），零新增 IR 节点（desugar 层）。✅ 2026-09-04
- **M3（中）** deref coercion：`&T`/`&mut T` 经 `deref` 自动强制（如 `&String` → `&str`、智能指针 → 内部引用）。
- **L1（低）** 差分对拍：`guard.field` / `guard.method()` 自动透传。✅ 2026-09-04（run-pass `p1_4_autoderef.rl`）

## 受影响组件
`rlyeh-typecheck`（字段/方法/索引解析回退 + coercion）、`rlyeh-std`（MutexGuard/`Box<dyn Trait>`）。

## 验证
- Rlyeh 侧 `MutexGuard` 经 `guard.x` 自动 `(*guard).x` 访问；`Box<dyn Trait>` 方法经 deref 调用。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 复审补遗：从智能指针/MutexGuard 表达依赖中拆出 |
| 2026-09-04 | M1 落地：`core.rl` 声明 `trait Deref { type Target; fn deref(&self) -> &Self::Target; }` 与 `trait DerefMut { type Target; fn deref_mut(&mut self) -> &mut Self::Target; }`（关联类型 `Target` 经 U2/W4/V3-A4 已支持，编译验证通过）。M2（字段/方法/索引访问失败回退插入 `*(x.deref())` 递归）待实现 |
| 2026-09-04 | M2 落地：字段/方法/索引解析失败时，对接收者递归插入 `x.deref()`（限深度 `MAX_DEREF_DEPTH=16`）自动解引用强制，零新增 IR 节点（复用 `MethodCall`）。实现点：`check_expr/mod.rs`（`make_deref_receiver`/`MAX_DEREF_DEPTH`）、`method.rs` `check_method_call`、`field.rs` `check_field_access(_inner)`、`index_enum.rs` `check_index(_inner)`。run-pass `p1_4_autoderef.rl` 验证（含嵌套强制链）。**已知限制**：`Deref` trait 的 `deref(&self) -> &Target`（返回引用）路径受「函数返回引用作为 place」既有 codegen 缺陷影响（`let r = x.deref(); r.field` 亦报错），故 M2 仅对**返回值**的 `deref`（对齐 std 智能指针 `MutexGuard`/`RwLockGuard` 的 `deref(&self) -> T` 分发）生效；`&Target` 返回路径待函数返回引用 codegen 修复后启用 |
