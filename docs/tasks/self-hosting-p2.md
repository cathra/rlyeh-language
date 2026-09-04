# P2 缺口索引 — 可行但需重写 / 外部依赖 / 集成建设

> **级别定义**：Rlyeh 0.1.0 已实现或接近实现，但自举时需重写算法、依赖外部 C 库或建设集成基础设施。P2-2/3/4/5/6/7 纳入 0.2.0；P2-1（外部 crate/dagon）长期保留 Rust 经 FFI。
> **根索引**：[`self-hosting.md`](./self-hosting.md) · **评估报告**：[`../self-hosting/feasibility.md`](../self-hosting/feasibility.md) §4 P2。

---

## 缺口清单

| ID | 缺口 | 0.2.0 归属 | 受影响组件 | 叶子 | 状态 |
|----|------|-----------|-----------|------|------|
| SH-P2-1 | 外部 crate 等价（pubgrub/clap/serde/toml/tar/flate2/libc） | 长期保留 Rust | dagon / driver | [leaf](./leaf/sh-p2-1-external-crates.md) | ⏳ 规划中 |
| SH-P2-2 | 进程调用 / 外部工具链 FFI | 0.2.0-D | rlyeh-driver `assemble()` | [leaf](./leaf/sh-p2-2-process-ffi.md) | ⏳ 规划中 |
| SH-P2-3 | `Box` 深树 + 内部可变性（arena/RefCell 等价） | 0.2.0-I | AST/HIR/MIR/LIR 可变遍历 | [leaf](./leaf/sh-p2-3-internal-mut.md) | ⏳ 规划中 |
| SH-P2-4 | FFI/ABI 链接桥（Rlyeh 产物链接 Rust 运行时） | 0.2.0-J | codegen + Rust 运行时 rlib | [leaf](./leaf/sh-p2-4-linkage-bridge.md) | ⏳ 规划中 |
| SH-P2-5 | 分阶段自举 + 差分测试基础设施 | 0.2.0-K | 引导器 + 测试 harness | [leaf](./leaf/sh-p2-5-staged-bootstrap.md) | ⏳ 规划中 |
| SH-P2-6 | 诊断信息质量对齐 | 0.2.0-L | typecheck / check 诊断 | [leaf](./leaf/sh-p2-6-diagnostics.md) | ⏳ 规划中 |
| SH-P2-7 | driver 自举（增量编译 / 线程 / 缓存） | 0.2.0-M/K | rlyeh-driver | [leaf](./leaf/sh-p2-7-driver.md) | ⏳ 规划中 |
| SH-P2-8 | `mem::swap` / `mem::replace` 内建 | 0.2.0-U | typecheck / borrowck / desugar / regionck | [leaf](./leaf/sh-p2-8-mem-swap.md) | ⏳ 规划中 |
| SH-P2-9 | `const` / `static` 全局项（编译期常量 + 全局符号） | 0.2.0-V | typecheck / codegen / 运行时 FFI | [leaf](./leaf/sh-p2-9-const-static.md) | ⏳ 规划中 |
| SH-P2-10 | `panic!` / `assert!` / `unreachable!` / `todo!` 宏 | 0.2.0-W | macro / typecheck / std | [leaf](./leaf/sh-p2-10-assert-macros.md) | ⏳ 规划中 |
| SH-P2-11 | 结构体 `..` 更新 + 字段简写 | 0.2.0-X | parser / typecheck / codegen | [leaf](./leaf/sh-p2-11-struct-update.md) | ⏳ 规划中 |

---

## 进度

P2-2/3/4/6/7 ⏳ 规划中（入 0.2.0 D/I/J/L/M）；P2-1 ⏳ 规划中（长期保留 Rust）。
P2-5 的 K 阶段（差分测试基础设施）✅ 部分落地：0.2.0 收敛为 harness 脚手架 + 单编译器运行行为快照基线（C0/C1/C2，详见 SH-P2-5）；三阶段自举 K-M1..K-M3 推迟至 0.3.0（需 Rlyeh 自写编译器）。

---

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P2 拆出为分级索引 |
| 2026-09-01 | 新增 SH-P2-4 链接桥、SH-P2-5 分阶段自举、SH-P2-6 诊断、SH-P2-7 driver 自举 |
