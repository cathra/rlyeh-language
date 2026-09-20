# SH-P2-4 FFI/ABI 链接桥（Rlyeh 产物链接 Rust 运行时）

> **级别**：P2（集成建设） · **状态**：✅ 已完成（C-ABI staticlib 链接桥已在代码中落地） · **归属**：0.2.0-J
> **索引**：[`../self-hosting-p2.md`](../self-hosting-p2.md) · **评估**：[`../../self-hosting/feasibility.md`](../../self-hosting/feasibility.md) §3.3/§3.4

## 目标
建立 Rlyeh 编译产物与现有 Rust 运行时（`actor-runtime` / `region-alloc` / `gc-runtime` / `rlyeh-std` 绑定层）之间的**链接桥**，使 0.3.0 自举出的 Rlyeh 编译器/程序能正确调用 Rust 实现的运行时。

## 技术细节
- 当前 Rlyeh codegen 发射 LLVM IR 文本，由 `rlyeh-driver` 的 `assemble()` 调 `clang` 链接（见 `crates/rlyeh-driver/src/lib.rs`）。运行时目前是 Rust rlib。
- 需建设（原评估**完全遗漏**的关键项）：
  - **符号/C ABI 对齐**：Rlyeh 发射的调用约定、符号命名须与 Rust rlib 的 C ABI 边界一致（Rust `extern "C"` / `#[no_mangle]` 导出）。
  - **链接编排**：`assemble()` 在调 `clang` 时把 Rlyeh 编译产物（IR/`.o`）与 Rust 运行时 rlib 一并链接。
  - **运行时入口约定**：Rlyeh `fn main` 与 Rust 运行时初始化（GC/region/actor 引导、argv 传递）的衔接。
- 验证即"Rlyeh 写的 hello world 经 Rlyeh codegen + Rust 运行时链接，运行输出正确"。

## 受影响组件
`rlyeh-codegen`（符号/ABI）、`rlyeh-driver`（`assemble` 链接编排）、`rlyeh-std`（运行时入口）、所有运行时 rlib。

## 验证
- 集成：Rlyeh 写的最小程序（用 `println!`/actor spawn）经 Rlyeh codegen + Rust 运行时链接运行，行为等价于 Rust 编译器产物。

## 状态
✅ 已完成（0.2.0 必须项，阶段 J）：C-ABI staticlib 链接桥已在代码中落地，无需新建架构。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 新增（评审发现：原评估遗漏 Rlyeh 产物链接 Rust 运行时的桥接设计） |
| 2026-09-20 | 核实现状：链接桥已在代码中落地（非规划态）。actor/gc/region 运行时三 crate 均为 `crate-type=["rlib","staticlib"]`；codegen 以 C-ABI `declare` 发射 `rlyeh_actor_*`/`rlyeh_gc_*`/`rlyeh_region_*` extern 符号；driver `assemble()` 经 `-L/-l` 链接 `librlyeh_*_runtime.a`（缺失跳过、按需提取），并据 `@rlyeh_actor_` 引用给出诊断；actor 调度器经 `rlyeh_actor_init` 引导、wasm 走静态符号表 `rlyeh_actor_resolve`。状态由 ⏳ 规划中 校正为 ✅ 已完成 |
