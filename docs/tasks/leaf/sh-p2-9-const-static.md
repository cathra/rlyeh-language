# SH-P2-9 `const` / `static` 全局项

> **级别**：P2 · **风险**：🟠 中 · **状态**：⏳ 规划中 · **归属**：0.2.0-V
> **索引**：[`../self-hosting-p2.md`](../self-hosting-p2.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.22

## 目标
支持编译期 `const` 常量（常量折叠）与 `static` / `static mut` 全局项（data 段符号，FFI 共享状态），供运行时 FFI 全局表（如 actor 解析表 `rlyeh_actor_resolve`、`__rlyeh_*` 平台内建注册）表达。

## 现状
- 模块级 `pub const PI: f64` 语法已存在于 CODEBUDDY 模块示例，但**编译期常量折叠不完整**；`static` / `static mut` 全局项**缺失**（grammar.md 无产生式）。

## 风险分解（→ 中/低危）
- **M1（中）** 模块级 `const` 编译期常量传播 + 折叠（表达式常量求值），替换现有「仅字面量 const」。
- **M2（中）** `static` / `static mut` 全局项：data 段符号发射，FFI 共享可变/不可变状态（依赖 E `unsafe` 边界约定访问 `static mut`）。
- **M3（中）** `&'static T` 生命周期值（配合 F/E 的 `'static` 约束）。
- **L1（低）** 差分对拍：全局表初始化与读取。

## 受影响组件
`rlyeh-typecheck`（const 求值/static 符号）、`rlyeh-codegen`（data 段发射）、运行时 FFI。

## 验证
- 单元：actor resolve 静态表 `static` 项初始化 + 跨函数读取；`const` 折叠结果正确。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 复审补遗：从运行时 FFI 全局状态依赖中拆出 |
