# 自举能力缺口任务树（P0 / P1 / P2 / P3）

> 本树细化 [`../self-hosting/feasibility.md`](../self-hosting/feasibility.md) 的能力缺口（§4 P0/P1/P2），按**阻塞度分级**拆为不同级别文件：
> - 本文件 = **根索引**：列 P0/P1/P2/P3 四级总览 + 进度。
> - `self-hosting-p0.md` / `self-hosting-p1.md` / `self-hosting-p2.md` = **分级索引**：列该级缺口清单 + 叶子链接 + 进度。
> - `leaf/sh-*` = **叶子层**：每个最小缺口一个文档（目标 / 风险 / 技术细节 / 受影响组件 / 验证 / 状态 / 变更记录）。
>
> **版本边界（2026-09-01 修正）**：0.2.0 = 自举能力补齐（P0 语言特性 + P1 + P2-2/3/4/5/6/7/8/9/10/11 + P3-1 均入 0.2.0）；0.3.0 = 工具链自举（用 0.2.0 能力重写工具链）。P2-1（外部 crate/dagon）长期保留 Rust 经 FFI。路线见 [`../development-plan-0.2.0.md`](../development-plan-0.2.0.md)。
> **复审补遗（2026-09-01）**：对 0.2.0 实现目标复审，新增 P0-5~P0-8（元组值/解构、`if let`、`match` 守卫、`Drop`——均为**前端 PoC 本身的前提**，原计划漏判）、P1-4~P1-6、`P2-8~P2-11`、`P3-1`，共 12 项。每个叶子均含**风险分解（高危→中/低危）**。

---

## 分级总览

| 级别 | 含义 | 自举阻塞度 | 0.2.0 归属 | 缺口数 |
|------|------|-----------|-----------|--------|
| **P0** | Rlyeh 0.1.0 完全没有的能力 | 阻塞全栈/运行时自举 + 前端 PoC | **0.2.0 必须项（语言特性）** | 8 |
| **P1** | 需新增语言/标准库特性 | 阻塞前端自举 | 0.2.0 必须项 | 6 |
| **P2** | 可行但需重写 / 外部依赖 / 集成建设 | 后端 assemble + IR 遍历 + 链接/自举/诊断 + 语言补全 | P2-2/3/4/5/6/7/8/9/10/11 入 0.2.0；P2-1 长期 | 11 |
| **P3** | 并发安全地基（可放宽） | 并发原语线程安全基线 | P3-1 入 0.2.0（告警式，非硬阻塞） | 1 |

---

## 缺口清单（按级）

### P0 — Rlyeh 0.1.0 完全没有（0.2.0 能力补齐）
（状态标记：🟢 完成 / ⏳ 规划中，详见 [P0 索引](./self-hosting-p0.md)）
- 🟢 [SH-P0-1 `unsafe` 块 / 裸指针 / `#[repr(C)]`](./leaf/sh-p0-1-unsafe.md) → 0.2.0-E
- 🟢 [SH-P0-2 跨函数边界闭包 + `move` + `'static`](./leaf/sh-p0-2-closure.md) → 0.2.0-F
- 🟢 [SH-P0-3 `dyn Trait` 含 `Self` 方法 + `Any` 类型擦除](./leaf/sh-p0-3-dyn-any.md) → 0.2.0-G
- 🟢 [SH-P0-4 并发原语（Arc<Mutex>/atomic/线程 spawn）](./leaf/sh-p0-4-concurrency.md) → 0.2.0-H
- 🟢 [SH-P0-5 元组值构造 + 解构（多返回值）](./leaf/sh-p0-5-tuple-value.md) → 0.2.0-N（复审补遗；复核后仅解构为真缺口）
- 🟢 [SH-P0-6 `if let` / `while let` 模式控制流](./leaf/sh-p0-6-if-let.md) → 0.2.0-O（复审补遗；语言此前完全缺失，parser 层 desugar 落地）
- ⏳ [SH-P0-7 `match` 守卫 + 范围/或模式](./leaf/sh-p0-7-match-guard.md) → 0.2.0-P（复审补遗）
- ⏳ [SH-P0-8 `Drop` trait / 析构 / RAII](./leaf/sh-p0-8-drop.md) → 0.2.0-Q（复审补遗；语言完全缺失）

### P1 — 需新增语言/标准库特性（0.2.0 必须项）
- [SH-P1-1 泛型 trait/impl 完整化](./leaf/sh-p1-1-generic-trait.md) → 0.2.0-A
- [SH-P1-2 trait derive 宏](./leaf/sh-p1-2-derive.md) → 0.2.0-C
- [SH-P1-3 嵌套模块 / `pub use` / `super`](./leaf/sh-p1-3-nested-module.md) → 0.2.0-B
- [SH-P1-4 `Deref`/`DerefMut` 用户类型自动解引用强制](./leaf/sh-p1-4-deref.md) → 0.2.0-R（复审补遗）
- [SH-P1-5 `Copy`/`Clone` 语义 + `#[derive(Copy)]` + `T: Copy` 约束](./leaf/sh-p1-5-copy-clone.md) → 0.2.0-S（复审补遗）
- [SH-P1-6 `?` 运算符经 `From`/`Into` 错误自动转换](./leaf/sh-p1-6-question-from.md) → 0.2.0-T（复审补遗）

### P2 — 可行但需重写 / 外部依赖 / 集成建设
- [SH-P2-1 外部 crate 等价（pubgrub/clap/serde/toml/tar/flate2/libc）](./leaf/sh-p2-1-external-crates.md)（长期保留 Rust）
- [SH-P2-2 进程调用 / 外部工具链 FFI](./leaf/sh-p2-2-process-ffi.md) → 0.2.0-D
- [SH-P2-3 `Box` 深树 + 内部可变性（arena/RefCell 等价）](./leaf/sh-p2-3-internal-mut.md) → 0.2.0-I
- [SH-P2-4 FFI/ABI 链接桥（Rlyeh 产物链接 Rust 运行时）](./leaf/sh-p2-4-linkage-bridge.md) → 0.2.0-J
- [SH-P2-5 分阶段自举 + 差分测试基础设施](./leaf/sh-p2-5-staged-bootstrap.md) → 0.2.0-K
- [SH-P2-6 诊断信息质量对齐](./leaf/sh-p2-6-diagnostics.md) → 0.2.0-L
- [SH-P2-7 driver 自举（增量编译/线程/缓存）](./leaf/sh-p2-7-driver.md) → 0.2.0-M/K
- [SH-P2-8 `mem::swap` / `mem::replace` 内建](./leaf/sh-p2-8-mem-swap.md) → 0.2.0-U（复审补遗）
- [SH-P2-9 `const` / `static` 全局项](./leaf/sh-p2-9-const-static.md) → 0.2.0-V（复审补遗）
- [SH-P2-10 `panic!` / `assert!` / `unreachable!` / `todo!` 宏](./leaf/sh-p2-10-assert-macros.md) → 0.2.0-W（复审补遗）
- [SH-P2-11 结构体 `..` 更新 + 字段简写](./leaf/sh-p2-11-struct-update.md) → 0.2.0-X（复审补遗）

### P3 — 并发安全地基（可放宽，非硬阻塞）
- [SH-P3-1 `Send`/`Sync` 自动 trait（放宽/标记）](./leaf/sh-p3-1-send-sync.md) → 0.2.0-Y（复审补遗；MVP 可告警式）

---

## 总进度

| 级别 | 状态 |
|------|------|
| P0 | ⏳ 规划中（**0.2.0 必须项（语言特性）**，对应阶段 E/F/G/H/N/O/P/Q；P0-5~P0-8 为复审发现的 PoC 前置缺口） |
| P1 | ⏳ 规划中（0.2.0 必须项，对应阶段 A/B/C/R/S/T） |
| P2 | ⏳ 规划中（P2-2/3/4/5/6/7/8/9/10/11 入 0.2.0；P2-1 长期保留 Rust） |
| P3 | ⏳ 规划中（P3-1 入 0.2.0，告警式，非硬阻塞） |

---

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-09-01 | 新建自举缺口任务树（P0/P1/P2 三级，9 叶子） |
| 2026-09-01 | 修正版本边界：P0 语言特性上移为 0.2.0 必须项；新增 P0-4 并发原语、P2-4 链接桥、P2-5 分阶段自举、P2-6 诊断、P2-7 driver 自举（共 14 叶子） |
| 2026-09-01 | **复审补遗**：0.2.0 实现目标复审，新增 P0-5~P0-8、P1-4~P1-6、P2-8~P2-11、P3-1（共 12 叶子）；修正 sh-p0-2 归属标注与计划一致；每个叶子新增「风险分解（高危→中/低危）」小节 |
