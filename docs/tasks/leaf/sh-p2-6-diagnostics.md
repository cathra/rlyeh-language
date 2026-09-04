# SH-P2-6 诊断信息质量对齐

> **级别**：P2（集成建设） · **状态**：🟡 进行中 · **归属**：0.2.0-L
> **索引**：[`../self-hosting-p2.md`](../self-hosting-p2.md) · **评估**：[`../../self-hosting/feasibility.md`](../../self-hosting/feasibility.md)（诊断质量未在缺口中单列）

## 目标
Rlyeh 版编译器复刻 Rust 参考实现的 **span 级诊断质量**（文件名/行/列/长度 + 结构化错误码 + 建议），保证自举后开发者体验不退化（评审发现原评估未单列此能力）。

## 技术细节
- 当前 Rlyeh 诊断已有 Span 概念（lexer/parser 阶段），但 `typecheck`/`borrowck`/`regionck` 的错误信息丰富度需对齐 Rust 版。
- 需建设：
  - **L1** span 级错误定位（多文件/宏展开后映射）。
  - **L2** 结构化诊断（稳定错误码 + 修复建议 + 相关 span 标注）。
  - **L3** 与 Rust 参考实现诊断文本**对拍**（同一错误输入，诊断结构等价）。

## 受影响组件
`rlyeh-typecheck` / `rlyeh-borrowck` / `rlyeh-regionck` / `rlyeh-parser` 诊断输出、`rlyeh-check`（lint）。

## 验证
- 对拍：错误用例经 Rust/Rlyeh 编译器产出诊断结构（错误码 + span）一致。

## 实现纪要（L0，2026-09-04）
- harness 新增 `diagnostics` 维度（`scripts/diff_harness.py`）：捕获 `rlyeh run <file>` 的 stderr 诊断文本（类型检查 / 借用检查错误），无论退出码均记为已获取，诊断文本本身即快照标的。
- 新增 `tests/snapshot-baseline-diag-probe.txt`（12 例 compile-fail 代表性用例，覆盖类型不匹配 / 方法未找到 / 未定义变量·函数 / 借用冲突 / 泛型 where 约束 / match 守卫兜底 / 元组解构元数 / `?` 非 Option / 未知字段 / trait 约束 / 类型联合收窄），仅存 `diagnostics` 维度快照。
- CI（`.github/workflows/ci.yml`）新增 `Diagnostics probe regression (diff harness)` 步骤。
- 验证：diagnostics 探针连续两次 `check` 均 12/0/0/0，无诊断级非确定性。
- 已知缺口（后续项）：当前诊断 span 坐标为**合并源码（含 std 前缀）坐标**，非用户文件坐标（如 `type-mismatch.rl` 报 `7155:18`，实为 std 预置偏移后的行号）。**L1（用户态 span 对齐）** 需让 typecheck/borrowck/regionck 诊断减去 `prelude_len` 还原为用户行号；**L2（结构化诊断：稳定错误码 + 修复建议）** 亦为后续项。本增量仅为 L3 诊断对拍建立 harness 侧回归网。

## 实现纪要（L1，2026-09-04）
- **typecheck 诊断行号对齐到用户文件坐标**（SH-P2-6 L1 核心落地）：`rlyeh run/build` 把 std 预置拼到用户源码前，致 `TypeError` 的 `Span.line/col` 为合并源码坐标。
  - `rlyeh-typecheck/src/error.rs`：抽取 `write_message(f, loc, err)` 复用诊断正文格式化；新增 `TypeError::to_string_with_offset(prelude_len, prelude_lines)`，仅对落在用户代码（`span.start >= prelude_len`）的错误减去预置行数 `prelude_lines`（`prelude` 以换行结尾，用户源码从下一行第 1 列起，列号不变），预置内部 / 编译器生成项错误保持原坐标。
  - `rlyeh-driver/src/lib.rs`：`source_with_std` 额外返回预置行数 `prelude_lines = prelude.matches('\n').count() + 1`，透传至 `full_pipeline_with_hints` / `emit_hir` / `emit_hir_user` / `IncrementalDriver` 的类型检查错误映射，统一改用 `e.to_string_with_offset(prelude_len, prelude_lines)`。
  - 实测 `type-mismatch.rl` 诊断由 `7155:18`（合并坐标）修正为 `4:18`（用户文件第 4 行 `let x: i64 = "hello";`），坐标正确。
- **borrowck / regionck 诊断**：其错误 `line/col` 当前恒为 0（HIR 未传播 Span，错误 `Display` 不带坐标前缀），本增量**不涉及**坐标对齐，属后续 Span 传播任务（L1 余量 / 列为独立项），不在本基线坐标对齐范围内。
- 重新生成 `tests/snapshots/.../diagnostics.txt` 基线（12 例），`check` 验证 12/0/0/0。

## 状态
🟡 进行中（L0 harness 诊断维度 + 探针基线已落地；L1 typecheck 用户态 span 已对齐；borrowck/regionck Span 传播、L2 结构化诊断待办）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 新增（评审发现：诊断质量对齐未在原评估缺口中单列） |
| 2026-09-04 | L0 落地：harness `diagnostics` 维度 + 诊断探针基线（12 例）+ CI 步骤；记录 L1 用户态 span / L2 结构化诊断为后续项 |
| 2026-09-04 | L1 落地：typecheck 诊断行号对齐用户坐标（`to_string_with_offset` + `prelude_lines` 透传）；重新生成诊断基线（12 例，check 12/0/0/0）；borrowck/regionck Span 传播与 L2 待办 |
