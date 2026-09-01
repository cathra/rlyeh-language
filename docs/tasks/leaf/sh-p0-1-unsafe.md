# SH-P0-1 `unsafe` 块 / 裸指针

> **级别**：P0（阻塞全栈自举） · **风险**：🔴 高 · **状态**：⏳ 规划中 · **归属**：0.2.0-E
> **索引**：[`../self-hosting-p0.md`](../self-hosting-p0.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.5

## 目标
为 Rlyeh 引入受控的 `unsafe` 语义与裸指针类型（`*const T` / `*mut T`），使运行时三件套（gc / region / actor）与 `rlyeh-std` 绑定层可用 Rlyeh 表达，解除"运行时必须保留 Rust"的死结。

## 技术细节
- 当前 Rlyeh 0.1.0 无 `unsafe` 块，无法表达手动内存管理。
- 受影响 Rust 代码（事实依据，来自 `crates/` 核查）：
  - `rlyeh-gc-runtime/src/lib.rs`：`*mut i64` 裸指针、`UnsafeCell`、`#[repr(C)]`、`unsafe impl`、仅 `libc::malloc`。
  - `rlyeh-region-alloc/src/{allocator,region,cabi,block,destructor}.rs`：`unsafe`×36，手动 bump 链表 + `#[repr(C)]` + C ABI。
  - `rlyeh-actor-runtime/src/ffi.rs`：`#![allow(unsafe_code)]`、`unsafe extern "C"`、`*const c_char`、`dlsym`/`GetProcAddress` 平台 FFI。
  - `rlyeh-std/src/nio/*.rs`：`unsafe`×16（poller.rs 10），`libc` 系统调用封装。
- 需设计：unsafe 块作用域、裸指针读写为受控操作、`#[repr(C)]` 内存布局、FFI 安全边界约定。

## 受影响组件
`rlyeh-gc-runtime`、`rlyeh-region-alloc`、`rlyeh-actor-runtime`、`rlyeh-std`（nio 绑定层）。

## 验证
- 单元：Rlyeh 侧用 `unsafe` + 裸指针封装一个手动 bump 分配器并通过测试。
- 对拍：行为等价于 `rlyeh-region-alloc` 的 bump 路径。

## 状态
⏳ 规划中（**0.2.0 必须项（语言特性）**，对应阶段 E；运行时*重写*属 0.3.0）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P0-1 拆出为叶子 |
| 2026-09-01 | 修正归属：由「0.3.0+ 长期跟踪」上移为 0.2.0-E（与计划 §1/§3.5 一致）；索引由 feasibility 评估改为 development-plan §3.5 |
