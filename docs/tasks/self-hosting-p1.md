# P1 缺口索引 — 阻塞前端自举（0.2.0 必须项）

> **级别定义**：需新增语言/标准库特性方能解锁前端自举。对应 0.2.0 路线阶段 A/B/C（[`../development-plan-0.2.0.md`](../development-plan-0.2.0.md)）。
> **根索引**：[`self-hosting.md`](./self-hosting.md) · **评估报告**：[`../self-hosting/feasibility.md`](../self-hosting/feasibility.md) §4 P1。

---

## 缺口清单

| ID | 缺口 | 对应 0.2.0 阶段 | 受影响组件 | 叶子 | 状态 |
|----|------|----------------|-----------|------|------|
| SH-P1-1 | 泛型 protocol / impl 完整化 | 0.2.0-A | typecheck（注释编译器自身需泛型 protocol） | [leaf](./leaf/sh-p1-1-generic-protocol.md) | 🟢 完成（A1/A3 经复核为既有能力；A4 两处缺口已补；A2 剩两处限制单独立项） |
| SH-P1-2 | protocol derive 宏 | 0.2.0-C | AST / HIR / MIR / LIR（`derive`×70+） | [leaf](./leaf/sh-p1-2-derive.md) | 🟢 核心落地 |
| SH-P1-3 | 嵌套模块 / `pub use` / `super` | 0.2.0-B | 所有大型 crate（driver / typecheck 多级模块） | [leaf](./leaf/sh-p1-3-nested-module.md) | 🟢 完成（含嵌套组导入，2026-09-04） |
| SH-P1-4 | `Deref` / `DerefMut` 用户类型自动解引用强制 | 0.2.0-R | typecheck / std（MutexGuard / `Box<dyn Protocol>`） | [leaf](./leaf/sh-p1-4-deref.md) | 🟢 M1+M2 落地（2026-09-04；`&Target` 返回路径待函数返回引用 codegen 修复） |
| SH-P1-5 | `Copy` / `Clone` 语义 + `#[derive(Copy)]` + `T: Copy` 约束 | 0.2.0-S | typecheck / std（基础类型） | [leaf](./leaf/sh-p1-5-copy-clone.md) | 🟢 完成（2026-09-04） |
| SH-P1-6 | `?` 运算符经 `From` / `Into` 错误自动转换 | 0.2.0-T | typecheck（`?` desugar）/ std | [leaf](./leaf/sh-p1-6-question-from.md) | 🟢 完成（P6c 2026-08-29 已落地 `?`+`From`） |

---

## 进度

P1 全部 🟢 完成或核心落地：
- SH-P1-1 泛型 protocol/impl ✅（A1–A4）
- SH-P1-2 protocol derive 宏 🟢 核心落地（Clone/PartialEq/Debug，`Copy` 由 SH-P1-5 补齐）
- SH-P1-3 嵌套模块系统 🟢 完成（含组导入 / glob / `pub use` / 嵌套组导入）
- SH-P1-4 `Deref`/`DerefMut` 自动解引用强制 🟢 M1+M2 落地（2026-09-04；返回值 deref 路径生效，`&Target` 返回路径待函数返回引用 codegen 修复）
- SH-P1-5 `Copy`/`Clone` + `#[derive(Copy)]` + `T: Copy` ✅
- SH-P1-6 `?`+`From` 错误自动转换 ✅（P6c 落地）

---

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P1 拆出为分级索引 |
| 2026-09-04 | SH-P1-4 M1+M2 落地：`Deref`/`DerefMut` protocol 声明 + 字段/方法/索引访问失败自动解引用强制（限深度） |
