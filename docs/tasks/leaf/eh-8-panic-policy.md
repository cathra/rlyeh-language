# EH-8 panic 策略与可认证性（呼应 TCL2/ASIL）

> **级别**：P1 · **风险**：🔴 高（安全相关） · **状态**：🟡 进行中（M4 已落地并验证；panic 策略文档/`panic=` 开关/`#[no_panic]` 待办） · **归属**：0.2.0-AA / TCL2
> **索引**：[`../rfc/error-handling.md`](../rfc/error-handling.md) §4.8 · **关联**：[`../rfc/tcl2-certification.md`](../rfc/tcl2-certification.md) §5.2

## 目标
为不可恢复失败定义**文档化、可配置、确定性**的 panic 策略，满足 ASIL B/C 与 TCL2 对"工具 bug 不逸出"的要求：
- `panic=abort` / `panic=unwind` 开关（安全关键路径默认 `abort`）。
- `#[no_panic]` 标注 + 静态分析（确保标注函数不含 panic 点）。
- 明确 `unwrap`/`expect` 失败、actor 返回 `-1`、OOM 的统一行为（确定性 abort + 最小诊断，不泄漏未初始化内存）。

## 现状
- panic 当前为 MVP 直接 abort，但**无文档化策略、无配置开关、无 `no_panic`**（RFC §1.6）。
- 直接 `arr[i]` 越界读写**原无任何边界检查**（UB），经本叶子 M4 已硬化。

## 风险分解
- **M1（高）** 文档化 panic 语义（`manual/std/result.md` 扩展）。
- **M2（高）** `panic=` 开关接入 driver + codegen（abort/unwind 选择）。
- **M3（中）** `#[no_panic]` 标注 + 静态检查（复用 `rlyeh-check`）。
- **M4（高）** 与 tcl2 §5.2 边界硬化协同：直接 `arr[i]` 越界改为确定性 panic（而非 UB）—— ✅ **已落地并验证（2026-09-20）**：`HirExprKind::Index/IndexSet` 新增 `len: Option<Box<HirExpr>>`，typecheck `check_index_inner` 为 Vec/`String`/`&str`/切片填入 `FieldGet{base,1}`（fat 指针长度槽）；经 MIR/LIR 传递到 `rlyeh-codegen/src/llvm/llvm_field.rs`，在 `len` 为 `Some` 时注入 `icmp slt/sge + br + call @abort()`。越界 `v[5]`（len=2）IR 生成 `idx_fail: call void @abort()` 并确定性中止；界内索引正常。回归测试 `tests/run-pass/vec_index_inbounds.rl`。裸指针/`Str`/固定数组暂未硬化（留待补）。

## 受影响组件
`rlyeh-driver`（开关）、`rlyeh-codegen`（abort/unwind 落地）、`rlyeh-check`（`no_panic` 检查）、`manual`。

## 验证
- 集成：标注 `#[no_panic]` 函数内出现 `unwrap` 触发编译错误；越界 `arr[i]` 在 `panic=abort` 下确定性终止（不 UB，已验证）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | 评审通过后建叶子；M4 越界硬化落地并验证（codegen 注入 `call @abort()`，IR + 运行时确认） |
