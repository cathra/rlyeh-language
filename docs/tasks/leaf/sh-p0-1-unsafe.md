# SH-P0-1 `unsafe` 块 / 裸指针

> **级别**：P0（阻塞全栈自举） · **风险**：🔴 高 · **状态**：🟡 部分完成（E-M1 + 验证已落地；E3 FFI 门禁已落地；E2 `#[repr(C)]` 真布局待专项） · **归属**：0.2.0-E
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
🟡 部分完成（**0.2.0 必须项（语言特性）** 核心已落地；运行时*重写*属 0.3.0）。

- ✅ **E-M1 地基（已实现并验证）**：`unsafe { }` 块表达式 + 裸指针（`*const T`/`*mut T`）读写与索引。
  - AST/HIR 新增 `UnsafeBlock` 变体；parser 解析 `unsafe { }`（`is_item_start` 区分 `unsafe fn` 项与 `unsafe { }` 表达式语句）；typecheck/MIR/borrowck/regionck/desugar/fmt/check 各匹配臂委托到块逻辑。
  - 指针算术 `*mut T + i64 → *mut T` 本就支持（typecheck `mod.rs` BinaryOp::Add 特判）。
  - 验证：`tests/run-pass/unsafe-raw-ptr.rl`（裸指针读写/索引）+ `tests/run-pass/unsafe-bump-allocator.rl`（手动 bump 分配器，对拍 `rlyeh-region-alloc` bump 路径）。
- 🟡 **E2 `#[repr(C)]` 内存布局**：基础设施已落地（属性解析 + `repr_c` 标志下传至 `AstStructDecl`）；默认布局与 C 兼容（8 字节对齐字段即 C 布局）。**真布局（sub-8 字节字段 C 打包）待专项**——需将字段真实尺寸自 HIR 经 MIR→LIR→codegen 下传（当前 `MirProgram` 不携带结构体字段类型，需补布局信息；codegen 为 i8\* 字节偏移模型，`FieldScalar` 已丢失字段尺寸），风险高，建议作为 0.3.0 运行时重写的前置专项。验证：`tests/run-pass/repr-c-struct.rl`。
- ✅ **E3 FFI 安全边界约定（extern 调用门禁已实现并验证）**：`extern fn` 调用强制要求在 `unsafe` 块内（typecheck `in_unsafe` 上下文 + `extern_fns` 查表）；标准库预置（prelude）经字节长度豁免受信任 FFI（类比 Rust std）。裸指针解引用门禁因会波及 `rlyeh-std` 裸指针用法、风险高，暂未强制（E-M1 已允许 `unsafe` 块内解引用）。验证：`tests/run-pass/unsafe-extern-call.rl` + `tests/compile-fail/unsafe-extern-call-outside.rl`。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P0-1 拆出为叶子 |
| 2026-09-01 | 修正归属：由「0.3.0+ 长期跟踪」上移为 0.2.0-E（与计划 §1/§3.5 一致）；索引由 feasibility 评估改为 development-plan §3.5 |
| 2026-09-01 | 实现 E-M1：`unsafe { }` 块 + 裸指针读写/索引（跨 ast/hir/parser/typecheck/mir/borrowck/regionck/desugar/fmt/check 多 crate）；新增 run-pass 用例 `unsafe-raw-ptr.rl`、`unsafe-bump-allocator.rl`，状态由规划中改为部分完成 |
| 2026-09-01 | 实现 E3 FFI 安全边界门禁：extern 调用须 `unsafe`（typecheck `in_unsafe` + `extern_fns` 查表；driver 注入 prelude 字节长度豁免 std 受信任 FFI）；新增 `unsafe-extern-call.rl`（run-pass）+ `unsafe-extern-call-outside.rl`（compile-fail） |
| 2026-09-01 | E2 基础设施：`#[repr(C)]` 属性解析（`parse_attributes` 扩展）+ `repr_c` 标志下传至 `AstStructDecl`；新增 `repr-c-struct.rl`（run-pass） |
