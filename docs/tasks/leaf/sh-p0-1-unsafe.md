# SH-P0-1 `unsafe` 块 / 裸指针

> **级别**：P0（阻塞全栈自举） · **风险**：🔴 高 · **状态**：🟢 完成（E-M1 + 验证已落地；E2 `#[repr(C)]` 真布局含嵌套聚合内联已落地；E3 FFI 门禁已落地） · **归属**：0.2.0-E
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
- ✅ **E2 `#[repr(C)]` 真布局（sub-8 字节标量 C 打包，已实现并验证）**：属性解析 + `repr_c` 标志已下传（`AstStructDecl`），并将真实字段尺寸 / 偏移 / 提升方式经 `FieldScalar::ReprCField { offset, field_ty, conv }` 自 HIR 经 MIR→LIR→codegen 下传（无需改动 250 处 `FieldGet/FieldSet` 构造，仅 ~6 个匹配臂 + 构造点）。
  - **字段访问**（`FieldGet`/`FieldSet`/`FieldAddr`）与**结构体字面量构造**（ctor `FieldSet`）均按 C 规则落位：自然对齐（size 上取整对齐）、紧密打包（a@0, b@4, c@8...），窄字段读经 `zext`/`sext`/`fpext` 提升为宽值、写经 `trunc`/`fptrunc` 降窄；聚合字段（嵌套结构体 / 枚举 / 数组 / 字符串视图 / dyn Trait）仍由**安全护栏**显式拒绝（`collect.rs`：`c_field_size(&ty).is_none()`）。
  - **裸指针字节语义（FFI 字节缓冲 / 结构体字节级校验）**：`*const u8`/`*mut u8` 的索引（`p[i]`）、算术（`p + n`）、解引用（`*p` 读 / `*p = v` 写）均按 **1 字节步长 + 1 字节读写**（zext/trunc 到宽值），与 C ABI 一致；其余元素（如 `*const i64`）维持 8 字节槽步长（兼容既有裸指针语义）。
  - 验证：`tests/run-pass/repr-c-struct.rl`（全 8 字节对齐字段，与 C 天然一致）、`tests/run-pass/repr-c-packing.rl`（sub-8 标量读 / 写转换：u8/u16/i32/u32/i64 边界值）、`tests/run-pass/repr-c-packing-bytes.rl`（裸指针逐字节读取确认 a@0、b@4 的 C 打包）、`tests/compile-fail/repr-c-sub8.rl`（聚合字段报错）。完整套件 214/214 通过。
- ✅ **E2 嵌套聚合内联打包（已实现并验证）**：repr(C) 结构体支持嵌套 repr(C) 结构体 / 枚举 / 数组字段按 C 规则内联打包——`compute_repr_c` 递归为嵌套聚合分配 `CField::Nested{offset,size}`（整体大小内联、自然对齐），`field_scalar_of` 对 repr(C) 命名类型产出 `ReprCSubPtr{offset,size}`；`FieldScalar` 新增 `ReprCSubPtr` 变体（HIR/LIR 贯通），`FieldGet` 经 GEP 返回指向 `offset` 的子指针、`FieldSet` 经 `llvm.memcpy` 整块拷入 `size` 字节（整体赋值嵌套字段），`FieldAddr` 同款 GEP。
  - **非 repr(C) 嵌套拒绝**：`compute_repr_c` 对未标注 `#[repr(C)]` 的嵌套结构体显式报错「嵌套结构体未标注 #[repr(C)]」（标量与非 repr(C) 嵌套仍走安全护栏）。
  - **字节级校验**：裸指针逐字节读取确认嵌套内联偏移——`Line{a:Point{x,y},b:i32,tag:u8}` ⇒ a.x@0/a.y@4/b@8/tag@12（size 16），`Scene{line:Line,extra:i32}` ⇒ line@0/extra@16（size 20）；三层下降 `Scene→Line→Point` 与子指针绑定（`let q = l.a; q.x`）均正确。
  - 验证：`tests/run-pass/repr-c-nested.rl`（嵌套读 / 写 / 子指针绑定 / 三层嵌套 / 字节级布局校验，22 行输出全绿）+ `tests/run-pass/repr-c-packing.rl` + `tests/run-pass/repr-c-packing-bytes.rl` + `tests/compile-fail/repr-c-sub8.rl`（非 repr(C) 嵌套拒绝）；完整套件 215/215 通过。
- ✅ **E3 FFI 安全边界约定（extern 调用门禁已实现并验证）**：`extern fn` 调用强制要求在 `unsafe` 块内（typecheck `in_unsafe` 上下文 + `extern_fns` 查表）；标准库预置（prelude）经字节长度豁免受信任 FFI（类比 Rust std）。裸指针解引用门禁因会波及 `rlyeh-std` 裸指针用法、风险高，暂未强制（E-M1 已允许 `unsafe` 块内解引用）。验证：`tests/run-pass/unsafe-extern-call.rl` + `tests/compile-fail/unsafe-extern-call-outside.rl`。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P0-1 拆出为叶子 |
| 2026-09-01 | 修正归属：由「0.3.0+ 长期跟踪」上移为 0.2.0-E（与计划 §1/§3.5 一致）；索引由 feasibility 评估改为 development-plan §3.5 |
| 2026-09-01 | 实现 E-M1：`unsafe { }` 块 + 裸指针读写/索引（跨 ast/hir/parser/typecheck/mir/borrowck/regionck/desugar/fmt/check 多 crate）；新增 run-pass 用例 `unsafe-raw-ptr.rl`、`unsafe-bump-allocator.rl`，状态由规划中改为部分完成 |
| 2026-09-01 | 实现 E3 FFI 安全边界门禁：extern 调用须 `unsafe`（typecheck `in_unsafe` + `extern_fns` 查表；driver 注入 prelude 字节长度豁免 std 受信任 FFI）；新增 `unsafe-extern-call.rl`（run-pass）+ `unsafe-extern-call-outside.rl`（compile-fail） |
| 2026-09-01 | E2 基础设施：`#[repr(C)]` 属性解析（`parse_attributes` 扩展）+ `repr_c` 标志下传至 `AstStructDecl`；新增 `repr-c-struct.rl`（run-pass） |
| 2026-09-01 | E2 安全护栏：typecheck 对含聚合字段的 repr(C) 结构体显式报错（仅拒聚合，sub-8 标量字段放行，待真布局专项）；新增 `repr-c-sub8.rl`（compile-fail） |
| 2026-09-01 | E2 真布局落地：sub-8 字节标量字段 C 打包（自然对齐 + 紧密打包），经 `FieldScalar::ReprCField` 下传偏移 / 内存类型 / 提升方式到 codegen；struct 字面量 ctor 与 `FieldGet`/`FieldSet`/`FieldAddr` 均按 C 偏移落位；裸指针 `*const u8`/`*mut u8` 索引 / 算术 / 解引用改为 1 字节步长 + 1 字节读写（FFI 字节缓冲语义）。新增 `repr-c-packing.rl`/`repr-c-packing-bytes.rl`（run-pass）；修正 `unsafe-bump-allocator.rl`（依赖旧 8 字节宽 u8 指针语义，改为字节合法值 250）；完整套件 214/214 通过 |
| 2026-09-01 | E2 嵌套聚合内联打包：repr(C) 支持嵌套 repr(C) 结构体 / 枚举 / 数组字段内联（`compute_repr_c` 递归 `CField::Nested` + `field_scalar_of` 产出 `ReprCSubPtr{offset,size}`）；`FieldScalar::ReprCSubPtr` 贯通 HIR/LIR，`FieldGet` GEP 返回子指针、`FieldSet` 经 `llvm.memcpy` 整块拷入、`FieldAddr` 同款；非 repr(C) 嵌套显式拒绝；codegen 收紧 `store_to`/`operand_value` 的 `%` 前缀约定（裸寄存器名传入 `store_to` 由其统一补 `%`）。新增 `repr-c-nested.rl`（run-pass，22 行全绿）；修正 `repr-c-sub8.rl`（Inner 去掉 `#[repr(C)]` 以真正触发拒绝）；完整套件 215/215 通过 |
