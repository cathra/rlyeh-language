# SH-P1-5 `Copy` / `Clone` 语义 + `#[derive(Copy)]` + `T: Copy` 约束

> **级别**：P1 · **风险**：🟠 中 · **状态**：🟢 完成 · **归属**：0.2.0-S
> **索引**：[`../self-hosting-p1.md`](../self-hosting-p1.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.19

## 目标
明确 `Copy`（隐式按位拷贝标记）与 `Clone`（`fn clone(&self) -> Self` 显式拷贝）语义，支持 `#[derive(Copy)]` / `#[derive(Clone)]` 宏与泛型约束 `T: Clone` / `T: Copy`，对齐 Rust 拷贝模型（编译器内部大量标量/聚合传值依赖）。

## 现状
- `Clone` trait 已在 `core.rl` 声明（`trait Clone { fn clone(&self) -> Self; }`，SH-P1-2 derive 框架依赖）；`Copy` 标记 trait 原缺失；`#[derive(Copy)]` 未展开；`T: Copy` 约束不可用。
- `From`/`Into` 的 `-> Self` 返回已由 SH-P0-3（dyn `Self`）/ SH-P1-1（A3）落地，`?`+`From` 转换已由 P6c 落地（见 SH-P1-6，实际已完成）。

## 风险分解（→ 中/低危）
- **M1（中）** `trait Copy {}` 标记 trait 声明（`core.rl`，无方法）。✅ 已完成
- **M2（中）** `#[derive(Copy)]` 扩展 C derive 框架：生成 `impl Copy for T {}`（零方法，标记 impl）。`#[derive(Clone, Copy)]` 两者并存生成。✅ 已完成
- **M3（中）** 泛型约束 `T: Copy` 检查：复用 SH-P1-1（A）的 bound 机制，经 `type_implements_trait` 命中派生 `impl Copy for T`。对用户 `#[derive(Copy)]` 类型生效。✅ 已完成
- **L1（低）** 差分对拍：聚合 `Copy` 与 Rust 一致（Rlyeh 默认按值拷贝、无 move 语义，故 Copy 主要作标记 / 约束，不改变运行时拷贝行为）。✅ 已完成

## 受影响组件
`rlyeh-typecheck`（`check_item/derive.rs` derive 展开）、`rlyeh-std`（`core.rl` trait 声明）。

## 验证
- `tests/run-pass/copy_clone.rl`：`#[derive(Copy)] struct Point` + 泛型 `fn identity<T: Copy>(v: T) -> T` 接收并原样返回；`p` 经传值后原绑定仍可用（不移动）；`#[derive(Clone, Copy)] struct Label` 同路径。输出 `3 / 4 / 3 / 7`。
- 全量 `rlyeh test tests` 无回归。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 复审补遗：从拷贝语义依赖中拆出 |
| 2026-09-04 | 🟢 完成：`trait Copy {}` 声明 + `#[derive(Copy)]` 展开为 `impl Copy for T {}` + `T: Copy` 约束经派生 impl 命中；run-pass 验证 |
