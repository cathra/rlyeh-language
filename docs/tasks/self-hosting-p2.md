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
| SH-P2-5 | 分阶段自举 + 差分测试基础设施 | 0.2.0-K | 引导器 + 测试 harness | [leaf](./leaf/sh-p2-5-staged-bootstrap.md) | 🟢 0.2.0 PoC(CD) |
| SH-P2-6 | 诊断信息质量对齐 | 0.2.0-L | typecheck / check 诊断 | [leaf](./leaf/sh-p2-6-diagnostics.md) | 🟢 完成（L0 harness 诊断维度 + 探针基线；L1 typecheck/borrowck/regionck 用户态 span 对齐 + 语句级坐标；L2 结构化诊断 TC/BC/RC0xx + help + 相关 span 标注） |
| SH-P2-7 | 前端自举 PoC（lexer/parser/ast/macro 用 Rlyeh 重写） | 0.2.0-M | rlyeh-driver / lexer / parser / ast / macro / tests | [leaf](./leaf/sh-p2-7-driver.md) | 🟡 进行中（M-M1a/b 切片1 落地：Rlyeh 版 lexer char/生命周期/not in/时间/原始字符串 + 差分对拍 harness 通过） |
| SH-P2-8 | `mem::swap` / `mem::replace` 内建 | 0.2.0-U | typecheck / borrowck / desugar / regionck | [leaf](./leaf/sh-p2-8-mem-swap.md) | 🟢 完成（M1 `mem::swap` 三次 memcpy 交换；M2 `mem::replace` desugar 复用 mem::swap 统一处理标量/聚合；M3 `mem::take` desugar 复用 mem::swap + 打通 `Default` 协议 `Self` 上下文推断，run-pass + compile-fail 已固化） |
| SH-P2-9 | `const` / `static` 全局项（编译期常量 + 全局符号） | 0.2.0-V | typecheck / codegen / 运行时 FFI | [leaf](./leaf/sh-p2-9-const-static.md) | 🟢 完成（M1 const 折叠 / M2 static·static mut data 段符号 + unsafe 门禁 TC016a/b/c / M3 `&GLOBAL`→`&'static T` 取址） |
| SH-P2-10 | `panic!` / `assert!` / `unreachable!` / `todo!` 宏 | 0.2.0-W | macro / typecheck / std | [leaf](./leaf/sh-p2-10-assert-macros.md) | 🟢 完成（panic!/unreachable!/todo!/assert!/assert_eq!/assert_ne! 全套 desugar 至内置 panic；run-pass + compile-pass 已固化） |
| SH-P2-11 | 结构体 `..` 更新 + 字段简写 | 0.2.0-X | parser / typecheck / codegen | [leaf](./leaf/sh-p2-11-struct-update.md) | 🟢 完成（L1 字段简写须显式首字段在前 / L2 `..base` 拷贝；run-pass 验证） |

---

## 进度

P2-2/3/4/7 ⏳ 规划中（入 0.2.0 D/I/J/M）；P2-1 ⏳ 规划中（长期保留 Rust）；P2-6 ✅ 完成（0.2.0-L 诊断质量对齐 L0/L1/L2 全落地）。
P2-5 的 K 阶段（差分测试基础设施）✅ 部分落地：0.2.0 收敛为 harness 脚手架 + 单编译器运行行为快照基线（C0/C1/C2，详见 SH-P2-5）；三阶段自举 K-M1..K-M3 推迟至 0.3.0（需 Rlyeh 自写编译器）。

---

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P2 拆出为分级索引 |
| 2026-09-01 | 新增 SH-P2-4 链接桥、SH-P2-5 分阶段自举、SH-P2-6 诊断、SH-P2-7 driver 自举 |
