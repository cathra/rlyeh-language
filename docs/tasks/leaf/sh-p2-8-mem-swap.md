# SH-P2-8 `mem::swap` / `mem::replace` 内建

> **级别**：P2 · **风险**：🟠 中 · **状态**：🟢 M1+M2 已完成、M3 暂缓（依赖 Default trait） · **归属**：0.2.0-U
> **索引**：[`../self-hosting-p2.md`](../self-hosting-p2.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.21

## 目标
提供 `mem::swap(&mut a, &mut b)` / `mem::replace(&mut a, b)` / `mem::take(&mut a)` 内建，供 IR 重写（borrowck / desugar / regionck）在不触发借用冲突的前提下交换/替换值（Rust 编译器大量使用 `std::mem::swap`/`replace`）。

## 现状
- Rlyeh 无 `mem` 模块；借用检查 G1 严格（活跃可变借用期间写被借用变量报 `BorrowConflict`），IR 重写需 swap 避免冲突。

## 风险分解（→ 中/低危）
- **M1（中）** ✅ `mem::swap(&mut a, &mut b)` 内建：交换两可变引用指向的槽内容（codegen 经栈上临时缓冲做三次 `llvm.memcpy` 交换，零分配）。typecheck 在调用点算出 `size_of(T)`（普通结构体/元组按每字段 8 字节槽、repr(C) 走紧凑布局、标量按真实字节）并注入内建 `mem_swap(ptr, ptr, size)`。run-pass 覆盖标量/结构体/数组。
- **M2（中）** ✅ `mem::replace(&mut a, b)` 内建：desugar 为 `{ let _tmp = b; mem::swap(&mut a, &mut _tmp); _tmp }`，复用 mem::swap 统一处理标量 / 聚合（规避 LIR 引用坍缩为 `Ptr` 的泛型返回难题），返回旧值。
- **M3（低）** `mem::take(&mut a)`：`replace(a, Default::default())` 等价（依赖 Default trait）。
- **L1（低）** borrowck/desugar 重写用例：树节点指针交换无借用冲突。

> **M3 暂缓说明**：`mem::take(&mut a)` = `mem::replace(a, Default::default())`，依赖 `Default` trait（Rlyeh 当前未实现）。`mem::replace`（M2）已实现，通过 desugar 复用 `mem::swap` 规避泛型返回难题，无需内建泛型返回能力。

## 受影响组件
`rlyeh-typecheck`（`mem` 内建接线）、`rlyeh-borrowck`（swap 豁免）、`rlyeh-desugar`/`rlyeh-regionck`（IR 重写）。

## 验证
- 单元：AST 节点 `swap` 子节点无借用冲突；`replace` 返回旧值正确。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 复审补遗：从 IR 重写依赖中拆出 |
| 2026-09-20 | M1 `mem::swap` 落地：typecheck 调用点算 `size_of(T)` 注入内建 `mem_swap(ptr, ptr, size)`，codegen 经栈临时缓冲三次 memcpy 交换；run-pass 覆盖标量/结构体/数组。M2/M3 因内建泛型返回能力缺失暂缓 |
| 2026-09-20 | M2 `mem::replace` 落地：typecheck desugar 为 `{ let _tmp = b; mem::swap(&mut a, &mut _tmp); _tmp }`（`_tmp` 可变），复用 mem::swap 统一处理标量/聚合，无需内建泛型返回；run-pass `mem_replace.rl` + compile-fail `mem-replace-type-mismatch.rl` 固化；全量回归 333/333。M3 `mem::take` 仍依赖 `Default` trait 暂缓 |
