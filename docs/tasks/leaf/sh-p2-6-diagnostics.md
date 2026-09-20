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
- 新增 `tests/snapshot-baseline-diag-probe.txt`（12 例 compile-fail 代表性用例，覆盖类型不匹配 / 方法未找到 / 未定义变量·函数 / 借用冲突 / 泛型 where 约束 / match 守卫兜底 / 元组解构元数 / `?` 非 Option / 未知字段 / protocol 约束 / 类型联合收窄），仅存 `diagnostics` 维度快照。
- CI（`.github/workflows/ci.yml`）新增 `Diagnostics probe regression (diff harness)` 步骤。
- 验证：diagnostics 探针连续两次 `check` 均 12/0/0/0，无诊断级非确定性。
- 已知缺口（后续项）：当前诊断 span 坐标为**合并源码（含 std 前缀）坐标**，非用户文件坐标（如 `type-mismatch.rl` 报 `7155:18`，实为 std 预置偏移后的行号）。**L1（用户态 span 对齐）** 需让 typecheck/borrowck/regionck 诊断减去 `prelude_len` 还原为用户行号；**L2（结构化诊断：稳定错误码 + 修复建议）** 亦为后续项。本增量仅为 L3 诊断对拍建立 harness 侧回归网。

## 实现纪要（L1，2026-09-04）
- **typecheck 诊断行号对齐到用户文件坐标**（SH-P2-6 L1 核心落地）：`rlyeh run/build` 把 std 预置拼到用户源码前，致 `TypeError` 的 `Span.line/col` 为合并源码坐标。
  - `rlyeh-typecheck/src/error.rs`：抽取 `write_message(f, loc, err)` 复用诊断正文格式化；新增 `TypeError::to_string_with_offset(prelude_len, prelude_lines)`，仅对落在用户代码（`span.start >= prelude_len`）的错误减去预置行数 `prelude_lines`（`prelude` 以换行结尾，用户源码从下一行第 1 列起，列号不变），预置内部 / 编译器生成项错误保持原坐标。
  - `rlyeh-driver/src/lib.rs`：`source_with_std` 额外返回预置行数 `prelude_lines = prelude.matches('\n').count() + 1`，透传至 `full_pipeline_with_hints` / `emit_hir` / `emit_hir_user` / `IncrementalDriver` 的类型检查错误映射，统一改用 `e.to_string_with_offset(prelude_len, prelude_lines)`。
  - 实测 `type-mismatch.rl` 诊断由 `7155:18`（合并坐标）修正为 `4:18`（用户文件第 4 行 `let x: i64 = "hello";`），坐标正确。
- 重新生成 `tests/snapshots/.../diagnostics.txt` 基线（12 例），`check` 验证 12/0/0/0。

## 实现纪要（L1 余量：borrowck / regionck 坐标对齐，2026-09-04）
- **函数级（非语句级）坐标对齐落地**：HIR 子节点（`HirExpr`/`HirStmt`/`HirParam`）设计上不携带 `Span`，精确语句级坐标需 ~100+ HIR 构造点改造（风险高，不宜单增量完成）。故取**函数定义 `HirItem.span`**（合并源码坐标）作为借用/区域错误坐标，经 `render(prelude_lines)` 减预置行数还原为用户文件坐标，与 typecheck 对齐到同一坐标系（函数为粒度下限）。
- 改动：
  - `rlyeh-borrowck/src/error.rs` / `rlyeh-regionck/src/error.rs`：构造器新增 `span: Span` 参数并记录 `line/col`；新增 `message()`（不含坐标前缀，供 `// expect:` 子串断言复用）与 `render(prelude_lines)`（减预置行数 + `line:col:` 前缀）；`Display` 调 `render(0)` 保持无偏移语义。
  - `rlyeh-borrowck/Cargo.toml` / `rlyeh-regionck/Cargo.toml`：新增 `rlyeh-lexer` 依赖（引入 `Span`）。
  - `rlyeh-borrowck/src/checker.rs` / `rlyeh-regionck/src/checker.rs`：新增 `cur_span: Span` 字段，在 `check_program`/`check_item` 进入函数体时置为 `item.span`；全部错误构造点透传 `self.cur_span`。
  - `rlyeh-driver/src/util.rs`：新增 `DiagnosticsText` protocol（`to_user_text(prelude_lines)`），`BorrowError`/`RegionError` 各自实现；`join_errors` 改为 `join_errors<T: DiagnosticsText>(errs, prelude_lines)`。
  - `rlyeh-driver/src/lib.rs`：`full_pipeline_with_hints` 对 borrowck/regionck 错误映射改用 `join_errors(errs, prelude_lines)`（复用既有 `prelude_lines`）。
- 验证：`borrow-conflict-mutmut.rl` → `4:1: borrow conflict: cannot mutably borrow ...`（main 第 4 行，函数级坐标）；region 未定义 → `1:1: region 'r` not found in current scope`；compile-fail 12 例 `// expect:` 片段校验无回归（仅增 `line:col:` 前缀，消息正文不变）。
- 已知缺口（后续项）：**语句级精确坐标**需 HIR 子节点 Span 传播（重构项）；**L2 结构化诊断**（稳定错误码 + 修复建议 + 相关 span 标注）仍待办。

## 实现纪要（L1 余量 + 语句级精确坐标：HIR Span 全量传播，2026-09-05）
- **语句级精确坐标落地（HIR Span 传播重构）**：彻底移除"函数级坐标"取舍，borrowck/regionck 错误现定位到具体表达式 / 语句 / 块。
  - `rlyeh-hir`：`HirExpr`/`HirStmt` 重构为结构体包装 `HirExprKind`/`HirStmtKind` 并携带 `span`；`HirBlock`/`HirParam` 增加 `span` 字段；`rlyeh-lexer` 新增 `Span::dummy()`。
  - `rlyeh-typecheck`：在 `infer_expr`（包装层，根 `HirExpr.span` 取自 `AstExpr.span`）、`check_stmt`（包装层，每个 `HirStmt.span` 取自语句内含 `AstExpr`）、`check_block_inner`（`HirBlock.span` 取自 `AstBlock.span`）全量填充真实 span；所有下游消费方（borrowck/regionck/mir）切换为 `.kind` scrutinee + `::new(..)` 构造。
  - `rlyeh-borrowck`/`rlyeh-regionck`：`check_expr`/`check_stmt`/`check_block` 入口将 `cur_span` 设为被查节点自身 `span`（取代原 `HirItem.span` 函数级回退）。
  - 坐标体系一致：源码以 `{prelude}\n{source}` 合并后词法分析，节点 span 与函数 span 同为合并坐标，`render(prelude_lines)` 减偏移仍有效；用户态坐标正确。
  - 实测 `borrow-conflict-mutmut.rl` 由 `4:1`（main 函数级）修正为 `7:14`（`let r2 = &mut x;` 冲突行）；其余 11 例 typecheck 诊断坐标不变。
- 重新生成 `tests/snapshots/.../diagnostics.txt` 基线（仅 `borrow-conflict-mutmut` 一例坐标变化），`check` 验证 12/0/0/0。
- 遗留：~~L2 结构化诊断~~ 已于 2026-09-05 落地（见下「L2 结构化诊断」实现纪要）。

## 实现纪要（L2 结构化诊断：稳定错误码 + 修复建议 + 相关 span 标注，2026-09-05）
- **稳定错误码**：`TypeError` / `BorrowError` / `RegionError` 各变体新增 `code()`，分别映射到 `TC0xx` / `BC0xx` / `RC0xx`（跨版本稳定，可作机器可读锚点）。错误文本渲染为 `行:列: error[TC006]: ...`（typecheck）与 `行:列: [BC003] ...`（borrow/region）。
- **修复建议**：各变体新增 `help()` 返回 `Option<&'static str>`，高频错误给出中文修复提示，渲染为次级 `= help:` 行。
- **相关 span 标注**：`BorrowError` / `RegionError` 变体新增 `related: Vec<(Span, String)>` 字段；borrowck 检查器在 `Borrow` 记录上登记创建处 `span`，冲突（`register_borrow` / 赋值冲突）时回指先前借用位置，渲染为次级 `= note: <标签> (行:列)` 行。
- **向后兼容**：旧 `Display` / `render` / `to_string_with_offset` 输出格式**保持不变**（regionck 单测依赖其精确字符串）；driver 改用新增 `render_structured` / `to_string_structured`，`join_errors` 以换行分隔多错误块。
- 验证：重新生成 `tests/snapshots/.../diagnostics.txt` 基线（12 例，均新增 `[CODE]` / `= help:` 行；`borrow-conflict-mutmut` 新增 `= note: 先前借用创建于此 (6:13)`），`check` 验证 12/0/0/0；`rlyeh test tests/` 全量 264/264 通过（`// expect:` 子串断言不受影响）。
- 已知缺口（2026-09-05 补齐 regionck 部分）：regionck 现已填充相关 span——`DoubleTransfer` 回指首次 transfer 处、`InvalidTransfer`/`OuterRegionTransfer` 回指目标区域声明处（`with_related` 已实际使用，移除 `#[allow(dead_code)]`）；`TypeError` 多位置标注已部分落地（见下「L2 续：TypeError 多位置标注」）：`WrongType`/`ArgumentTypeMismatch` 新增 `related` 并渲染偏移感知 `= note:`，结构体字段不匹配回指字段声明、返回类型不匹配回指函数/方法声明；`let` 注解 / 函数实参 / 元组解构 / 枚举命名字段回指均已落地（2026-09-06，见变更记录）——`AstPattern::Tuple` 携带模式 span、`VariantDef` 新增 `field_spans`，分别回指解构模式处与变体字段声明处；`RegionNotFound`/`PartialTransfer`/`RegionEscape` 无自然第二位置，保持单位置。

## 实现纪要（L2 续：TypeError 多位置标注，2026-09-05）
- **`TypeError` 多位置标注落地（SH-P2-6 文档记录的下一步「跟踪期望类型来源 span」）**：沿用 L2 既有 `= note:` 相关位置范式。
  - `rlyeh-typecheck/src/error.rs`：`WrongType` / `ArgumentTypeMismatch` 变体新增 `related: Vec<(Span, String)>` 字段；`to_string_structured` 在稳定错误码 + `= help:` 之后，对 `related` 逐条渲染偏移感知的 `= note: <标签> (行:列)`（坐标还原逻辑与 `loc` 一致：落在预置范围内保持原坐标，用户代码减预置行数）。
  - 全部 21+ 处 `WrongType` / `ArgumentTypeMismatch` 构造点补 `related`（填充或 `vec![]`）。
  - **已填充回指（可零改造获取声明 span 的位置）**：
    - 结构体字段类型不匹配（`construct.rs`）：回指字段声明处，span 取自新增的 `StructDef.field_spans`（`collect.rs` 由 `AstStructField.span` 收集，与 `fields` 同序）。
    - 返回类型不匹配（`fn_sig.rs` 函数 / `actor.rs` 方法）：回指函数/方法声明 span（`f.span`/`m.span`）。
    - 函数实参类型不匹配（`call.rs` 直接/间接调用）：回指第 `i+1` 个形参声明处，span 取自 `FnSignature.param_spans`（`fn_sig.rs` 由 `AstParam.span` 收集，与 `params` 同序；合成签名 / 无源码位置填 `Span::dummy()`，调用点按 dummy 跳过 note，避免 `(0:0)` 噪音）。
  - **元组解构 / 枚举命名字段回指已落地（2026-09-06）**：`AstPattern::Tuple` 由 `Tuple(Vec<AstPattern>)` 改为 `Tuple(Vec<AstPattern>, Span)`（parser 解析时记录 `(..)` 模式整体 span），`check_stmt` 元组解构不匹配（非元组 / 元数不符）补 `= note: 元组解构模式声明于此`；`VariantDef` 新增 `field_spans`（`collect.rs` 由 `AstEnumVariant.struct_fields[i].span` / 元组域回退变体 `v.span` 收集，与 `fields` 同序），`check_variant_construct` 字段类型不匹配补 `= note: 字段 \`fN\` 类型 \`T\` 声明于此`；合成元组模式（desugar `for` 注入）span 填 `Span::dummy()`，渲染按 dummy 跳过。`let x: T = expr` 注解处回指：将 `AstStmt::Let.type_anno` 由 `Option<AstType>` 改为 `Option<SpannedAstType>`（`rlyeh_ast` 新增 `SpannedAstType { ty, span }`，parser 解析时记录 `T` 处 span），`check_stmt` 类型标注不匹配补 `= note: 类型标注 \`T\``；合成注解 span 填 `Span::dummy()`，渲染按 dummy 跳过。
- 验证：新增回归用例 `tests/compile-fail/struct-field-type-mismatch.rl`（`// expect:` 同时断言 `= note: 字段 \`x\` 类型 \`i64\` 声明于此`）；`rlyeh test tests/` 全量 265/265 通过；diagnostics 探针基线 12/0/0/0（12 例均不含结构体字段/返回类型不匹配，无漂移）。

## 状态
✅ 完成（L0 harness 诊断维度 + 探针基线已落地；L1 typecheck 用户态 span 已对齐；L1 余量 borrowck/regionck 坐标已对齐至语句级（HIR Span 传播，2026-09-05）；L2 结构化诊断（稳定错误码 + 修复建议 + 相关 span 标注）已于 2026-09-05 落地；L2 续 TypeError 多位置标注已落地——结构体字段/返回类型/函数实参/let 注解/元组解构/枚举命名字段均回指声明或标注处；`RegionNotFound`/`PartialTransfer`/`RegionEscape` 无自然第二位置，保持单位置）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 新增（评审发现：诊断质量对齐未在原评估缺口中单列） |
| 2026-09-04 | L0 落地：harness `diagnostics` 维度 + 诊断探针基线（12 例）+ CI 步骤；记录 L1 用户态 span / L2 结构化诊断为后续项 |
| 2026-09-04 | L1 落地：typecheck 诊断行号对齐用户坐标（`to_string_with_offset` + `prelude_lines` 透传）；重新生成诊断基线（12 例，check 12/0/0/0）；borrowck/regionck Span 传播与 L2 待办 |
| 2026-09-04 | L1 余量落地：borrowck/regionck 函数级坐标对齐（`Span` 透传 + `render(prelude_lines)` 前缀；`DiagnosticsText` protocol + `join_errors` 透传 `prelude_lines`）；compile-fail 12 例片段校验无回归 |
| 2026-09-05 | 语句级精确坐标落地：HIR Span 全量传播（`HirExpr`/`HirStmt`/`HirBlock` 携带 `span` + typecheck 填充 + borrowck/regionck 节点级 `cur_span`）；`borrow-conflict-mutmut` 基线由 `4:1` 修正为 `7:14`；diagnostics `check` 12/0/0/0 |
| 2026-09-05 | L2 结构化诊断落地：稳定错误码（`TC/BC/RC0xx`）+ 修复建议（`= help:`）+ 相关 span 标注（`= note:`，borrow 冲突回指先前借用位置）；driver 改用 `render_structured`/`to_string_structured`，`join_errors` 换行分隔；diagnostics 基线重生成（12 例），`check` 12/0/0/0，`rlyeh test tests/` 264/264 |
| 2026-09-05 | L2 遗留补齐：regionck 填充相关 span（`DoubleTransfer` 回指首次 transfer 处、`InvalidTransfer`/`OuterRegionTransfer` 回指目标区域声明处），`with_related` 实际使用、移除 `#[allow(dead_code)]`；新增 regionck 单测断言 `= note:` 相关位置标注 |
| 2026-09-05 | L2 续：TypeError 多位置标注部分落地——`WrongType`/`ArgumentTypeMismatch` 加 `related` + 偏移感知 `= note:`，结构体字段/返回类型回指声明处；`StructDef.field_spans` 贯穿字段 span；let/实参/元组/枚举命名字段因 `AstType`/`AstStmt::Let`/`FnSignature`/`ResolvedVariant` 缺失注解 span 留作后续；新增回归用例，全量 265/265、诊断基线 12/0/0/0 |
| 2026-09-06 | L2 续：函数实参类型不匹配回指形参声明——`FnSignature` 加 `param_spans`（真实声明处填 `AstParam.span`，合成签名填 `Span::dummy()`）；`call.rs` 直接/间接调用实参不匹配按 dummy 跳过 note；新增回归用例 `func-arg-type-mismatch.rl`；全量 265/265、诊断基线 12/0/0/0 无漂移 |
| 2026-09-06 | L2 续：let 类型标注不匹配回指注解处——`AstStmt::Let.type_anno` 由 `Option<AstType>` 改为 `Option<SpannedAstType>`（`rlyeh_ast` 新增 `SpannedAstType { ty, span }`，parser 解析 `let x: T = e` 时记录 `T` 处 span），`check_stmt` 两处 `WrongType`（dyn 注解非 Fn / 标注不匹配）补偏移感知 `= note: 类型标注 \`T\``（dummy 跳过合成注解噪音）；重写/转发点（`desugar` `LetInfo` / `type_anno.clone()`、formatter `rlyeh-fmt`、6 处合成 `Some(..)`）同步适配；新增回归用例 `let-type-anno-mismatch.rl`，重生成 `cast`/`union_basics` 的 `ast-user` 快照；全量 265/265、诊断基线 12/0/0/0 无漂移 |
| 2026-09-06 | L2 续：元组解构绑定类型不匹配回指解构模式——`AstPattern::Tuple` 由 `Tuple(Vec<AstPattern>)` 改为 `Tuple(Vec<AstPattern>, Span)`（parser 解析记录 `(..)` 模式整体 span），`check_stmt` 非元组 / 元数不符两处 `WrongType` 补偏移感知 `= note: 元组解构模式声明于此`；desugar/序列化/格式化等 9 处匹配与 2 处合成 `Some(..)` 同步适配（合成填 `Span::dummy()`）；新增回归用例 `tuple-destructure-type-mismatch.rl` |
| 2026-09-06 | L2 续：枚举变体字段构造类型不匹配回指字段声明——`VariantDef` 新增 `field_spans`（`collect.rs` 由 `AstEnumVariant.struct_fields[i].span` / 元组域回退变体 `v.span` 收集，与 `fields` 同序），`check_variant_construct` 字段类型不匹配补偏移感知 `= note: 字段 \`fN\` 类型 \`T\` 声明于此`；新增回归用例 `enum-variant-field-mismatch.rl`；全量 265/265、诊断基线 12/0/0/0 无漂移 |
