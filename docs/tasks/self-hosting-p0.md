# P0 缺口索引 — Rlyeh 0.1.0 完全没有的能力（0.2.0 能力补齐）

> **级别定义**：Rlyeh 0.1.0 完全没有的能力，阻塞全栈/运行时自举。**2026-09-01 修正**：这些为**语言特性**，必须在 **0.2.0 能力补齐阶段**落地，使 0.3.0 能用 Rlyeh 自举语言本身；运行时 crate（actor/region/gc/std 绑定层）的*重写*属 0.3.0 工作，可长期保留 Rust 经 FFI 调用。
> **根索引**：[`self-hosting.md`](./self-hosting.md) · **评估报告**：[`../self-hosting/feasibility.md`](../self-hosting/feasibility.md) §4 P0。

---

## 缺口清单

| ID | 缺口 | 0.2.0 阶段 | 受影响组件 | 叶子 | 状态 |
|----|------|-----------|-----------|------|------|
| SH-P0-1 | `unsafe` 块 / 裸指针 / `#[repr(C)]` | 0.2.0-E | gc-runtime / region-alloc / actor-runtime ffi / rlyeh-std nio | [leaf](./leaf/sh-p0-1-unsafe.md) | 🟢 完成 |
| SH-P0-2 | 跨函数边界闭包 + `move` + `'static` | 0.2.0-F | actor-runtime / driver 线程模型 | [leaf](./leaf/sh-p0-2-closure.md) | 🟢 完成 |
| SH-P0-3 | `dyn Trait` 含 `Self` 方法 + `Any` 类型擦除 | 0.2.0-G | actor 消息协议 | [leaf](./leaf/sh-p0-3-dyn-any.md) | 🟢 完成 |
| SH-P0-4 | 并发原语（Arc<Mutex>/atomic/线程 spawn） | 0.2.0-H | actor-runtime / driver 并发 | [leaf](./leaf/sh-p0-4-concurrency.md) | ⏳ 规划中 |
| SH-P0-5 | 元组值构造 + 解构（多返回值） | 0.2.0-N | lexer / parser（PoC 重写）、typecheck / codegen | [leaf](./leaf/sh-p0-5-tuple-value.md) | ⏳ 规划中 |
| SH-P0-6 | `if let` / `while let` 模式控制流 | 0.2.0-O | lexer / parser / typecheck（PoC 重写） | [leaf](./leaf/sh-p0-6-if-let.md) | ⏳ 规划中 |
| SH-P0-7 | `match` 守卫 + 范围/或模式 | 0.2.0-P | parser / typecheck（字符分类/判别分支） | [leaf](./leaf/sh-p0-7-match-guard.md) | ⏳ 规划中 |
| SH-P0-8 | `Drop` trait / 析构 / RAII | 0.2.0-Q | typecheck / codegen / std（MutexGuard/arena/智能指针） | [leaf](./leaf/sh-p0-8-drop.md) | ⏳ 规划中 |

---

## 进度

P0 共 8 项：**SH-P0-1 / SH-P0-2 / SH-P0-3 🟢 完成**，其余 SH-P0-4~8 ⏳ 规划中（**0.2.0 必须项（语言特性）**，对应阶段 G/H/N/O/P/Q）。

---

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P0 拆出为分级索引 |
| 2026-09-01 | 修正：P0 上移为 0.2.0 必须项（语言特性）；新增 SH-P0-4 并发原语 |
| 2026-09-01 | SH-P0-1 状态由 ⏳ 规划中 更新为 🟢 完成（E-M1 / E2 真布局含嵌套聚合内联 / E3 FFI 门禁均落地） |
| 2026-09-01 | SH-P0-2 状态由 ⏳ 规划中 更新为 🟢 完成（F-M1/F-M2 `move` 字段化/F-M3 `'static` 校验/F-M4 `Thread::start(move || ..)` 跨线程执行；完整套件 218/218 通过） |
