# SH-P1-5 `Copy` / `Clone` 语义 + `#[derive(Copy)]` + `T: Copy` 约束

> **级别**：P1 · **风险**：🟠 中 · **状态**：⏳ 规划中 · **归属**：0.2.0-S
> **索引**：[`../self-hosting-p1.md`](../self-hosting-p1.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.19

## 目标
明确 `Copy`（隐式按位拷贝标记）与 `Clone`（`fn clone(&self) -> Self` 显式拷贝）语义，支持 `#[derive(Copy)]` / `#[derive(Clone)]` 宏与泛型约束 `T: Clone` / `T: Copy`，对齐 Rust 拷贝模型（编译器内部大量标量/聚合传值依赖）。

## 现状
- Rlyeh 默认值语义（标量拷贝）；`Clone` trait 未显式定义，`Copy` 标记缺失；derive 框架（C）仅含 Debug/Clone/PartialEq 标注，未覆盖 `Copy`。
- `From`/`Into` 泛型 trait 可解析但 `-> Self` 未支持（std-lib.md M2b）。

## 风险分解（→ 中/低危）
- **M1（中）** `trait Clone { fn clone(&self) -> Self; }` + `trait Copy {}` 标记 trait 声明（依赖 G `Self` 返回 / A 泛型）。
- **M2（中）** `#[derive(Clone)]` / `#[derive(Copy)]` 扩展 C derive 框架，自动生成 `clone`（逐字段克隆）/ `Copy` 标记。
- **M3（中）** 泛型约束 `T: Clone` / `T: Copy` 检查（扩展 A 的 `where`/`:` 机制），对未实现 trait 的类型报错。
- **L1（低）** 差分对拍：标量/聚合 `Copy` 与 Rust 一致；`clone` 深拷贝。

## 受影响组件
`rlyeh-typecheck`（trait/约束/derive）、`rlyeh-std`（基础类型 impl）。

## 验证
- 单元：`#[derive(Copy)] struct Point { x: i64, y: i64 }` 传值不移动；`#[derive(Clone)]` 深拷贝。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 复审补遗：从拷贝语义依赖中拆出 |
