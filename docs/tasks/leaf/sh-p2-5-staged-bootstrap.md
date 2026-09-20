# SH-P2-5 分阶段自举 + 差分测试基础设施

> **级别**：P2（集成建设） · **风险**：🔴 高 · **状态**：⏳ 规划中 · **归属**：0.2.0-K
> **索引**：[`../self-hosting-p2.md`](../self-hosting-p2.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.11

## 目标
建立「Rust 引导器编译 Rlyeh 版组件 → 三阶段 bootstrap 校验 → 差分测试 harness → 快照测试」的自举与对拍基础设施，作为 0.3.0 自举的工程骨架（0.2.0 内先落地 harness 与 PoC 级对拍）。

## 技术细节
- Rust 版编译器始终作为 bootstrap 编译器，先编译 Rlyeh 写的 `lexer`/`parser`/…/`typecheck`/`codegen`/`driver`。
- 差分 harness：同一 `.rl` 程序分别经 Rust 参考编译器与 Rlyeh 编译器编译，比较 IR 文本 / 可执行行为 / 诊断输出。
- 快照测试：AST/HIR/MIR/LIR 关键节点序列化快照，回归比对。
- 三阶段 bootstrap：① Rust 编译 Rlyeh 编译器；② Rlyeh 编译器编译自身得 `rlyeh₂`；③ `rlyeh₂` 编译自身得 `rlyeh₃`，`rlyeh₂` 与 `rlyeh₃` 字节/行为一致（经典自举校验）。

## 0.2.0 收敛范围与 checkpoint（2026-09-04 细化）

> 现实约束：Rlyeh 自写编译器尚不存在 → 真正的「Rust 参考 vs Rlyeh 编译器」跨编译器对拍属 0.3.0；0.2.0 仅落 **harness 脚手架 + 单编译器快照对拍**（以 Rust 编译器自身产物作参考快照），管道就位后未来无缝升级为双编译器对拍。
>
> 体积约束（实测，2026-09-04）：`--emit` 产出的 AST/HIR/IR 文本被 **std 前缀严重主导**——单程序 AST ~21 万行、HIR ~3.4 万行、IR ~1.8 万行，几乎全是前缀，逐程序存储不现实。因此 0.2.0 的**存储式快照基线以 `run`（运行行为）输出为主**（体积小、语义回归信号强）；AST/HIR/IR 三维作为**未来双编译器 `--rlyeh-b` 实时差分维度**（比对两份产物、无需存储）保留。若需 AST/HIR 逐程序快照，可经 `ast-user`/`hir-user` 维度实现（emit 时排除 std 前缀，C2 扩充·路线 A 已落地）；full `ast`/`hir` 仍保留用于人工审视。

- **C0（前置使能，✅ 已落地 2026-09-04）** driver `--emit <ir|ast|hir>`：停在中间表示文本、导出 stdout 或 `-o`，不调 clang 链接。复用 `compile_file_to_llvm` / `emit_ast` / `emit_hir`（AST/HIR 经 `Debug` `{:#?}` 文本化）。
- **C1（K4 harness，✅ 已落地 2026-09-04）** 对拍 harness 脚手架 `scripts/diff_harness.py`：跑 `rlyeh` 二进制 → 捕获 {运行行为 / IR / AST / HIR 文本} → 与 golden 快照比对；`--rlyeh-b` 可接入第二编译器触发双编译器差分（未来 Rlyeh 编译器就位即用）。
- **C2（K5 快照，✅ 已落地 2026-09-04）** 用 C1 对 `tests/` 精选子集生成**运行行为（run）快照基线**并接入 CI 回归比对。基线范围因 AST/HIR/IR 体积约束收敛为 `run` 维度（见上）；AST/HIR/IR 留作未来双编译器差分维度。

### 明确推迟到 0.3.0
- K-M1/K-M2（引导器编译 Rlyeh 版 lexer/parser / 双组件）→ 需 Rlyeh 源先存在。
- K-M3（三阶段 bootstrap `rlyeh₂ ≡ rlyeh₃`）。
- MIR/LIR 文本快照（需为各 IR 加序列化；C0 先只做 LLVM IR + AST + HIR）。

## 实现纪要（C0，2026-09-04）
- driver 新增库级 API `emit_ast(entry)` / `emit_hir(entry)`（`rlyeh-driver/src/lib.rs`）：
  - `emit_ast` 经 `module::load_combined_source` + `source_with_std` 取合并源码 → `rlyeh_parser::parse` → `format!("{:#?}", ast)`。
  - `emit_hir` 取合并源码 → `rlyeh_typecheck::typecheck_source_with_region_hints` → `format!("{:#?}", hir)`。
  - LLVM IR 复用既有 `compile_file_to_llvm`。
- CLI `main.rs`：新增 `EmitTarget` 枚举（`ir`/`ast`/`hir`）+ `CliOpts.emit` + `--emit` 解析 + `handle_emit` 短路（`run`/`build` 指定 `--emit` 时仅导出文本，不编译运行；`-o` 落盘，否则 stdout）。
- 烟测通过：`examples/arith-print.rl --emit ast|hir|ir` 均正确导出对应文本。

## 实现纪要（C1，2026-09-04）
- 新增 `scripts/diff_harness.py`（贴合项目 `scripts/*.py` 既有 Python 工具约定）：
  - 四维度捕获：`ir`/`ast`/`hir`（`build --emit`）+ `run`（行为 stdout）。
  - `check` 模式：与 `tests/snapshots/<relpath>/<dim>.txt` 比对；`update` 模式：写/更新基线。
  - 预留 `--rlyeh-b <另一编译器>`：启用双编译器差分（同维度逐文件比对，不读快照），未来 Rlyeh 自写编译器就位即无缝升级。
  - 默认扫描 `tests/run-pass` + `examples`；支持单文件与目录；`--timeout` 防挂起。
- **IR 非确定性（✅ 根因已修，2026-09-04）**：初版 LLVM IR 在两轮编译间 `alloca` 行序互换，源于 `LlvmEmitter.by_value_locals` 内层为 `HashSet<String>`（迭代顺序不确定）。已将其及 `propagate_by_value_aliases` 形参改为 `BTreeSet<String>`，发射顺序稳定（按变量名排序）。修复后两轮 `--emit ir` 逐字节一致，harness 现对 IR 维度**精确比对**（不再行排序变通）。此修复同时提升编译器可复现构建能力。
- 烟测通过：`update` + 连续两次 `check` 对 `examples/hello-world.rl` / `arith-print.rl` 均 4 维度全过、跨次零差异；两轮 `--emit ir` 直接 `diff` 零差异。

## 实现纪要（C2，2026-09-04）
- harness 增强 `scripts/diff_harness.py`：
  - `--dims <csv>`：维度子集（默认 `ir,ast,hir,run`），`capture` 仅取所需维度。
  - `--manifest <file>`：基线清单（每行一个 `.rl` 相对路径，`#` 注释；优先于 dirs 扫描），使基线可复现、可评审。
- 新增 `tests/snapshot-baseline.txt`：精选 ~30 个确定性运行行为用例（覆盖控制流/模式匹配/枚举/泛型/protocol/闭包/内存布局等），均为无并发、无哈希序依赖、无 IO 副作用的程序，作为语义回归轻量基线。
- 生成基线：`update --manifest tests/snapshot-baseline.txt --dims run` → 30 个 `tests/snapshots/<relpath>/run.txt`，总体积 120K（run 输出小）；连续三次 `check` 均 30/0/0/0，无行为非确定性。
- CI（`.github/workflows/ci.yml`）新增 `Snapshot regression (diff harness)` 步骤：`python3 scripts/diff_harness.py check --manifest tests/snapshot-baseline.txt --dims run`，编译回归即失败。
- 清理：删除冒烟期遗留的 `tests/snapshots/examples/`（含修复前、被前缀主导的 ast/hir/ir 大文件）。

## 实现纪要（C2 扩充，2026-09-04 续）
- **run 基线扩充**：`tests/snapshot-baseline.txt` 用例由 ~30 增至 **56**。新增 26 个经「连续两次 `rlyeh run` 输出逐字节一致」验证的确定性用例，覆盖：元组/解构、结构体/联合字段、repr-c 嵌套/结构体布局、切片、变量作用域/自返回、fn_ptr/assoc_type、Into 转换、unsafe/指针（addr_of/raw_ptr/extern-call/bump-allocator）、字符串/Vec API。全部无并发、无哈希序依赖、无 IO 副作用。
- **新增 IR 探针**（小子集，应对 AST/HIR/IR 被 std 前缀主导的体积约束）：
  - 新增 `tests/snapshot-baseline-ir-probe.txt`：精选 **6 例**代表性程序（hello 入口锚点 / arith 算术控制流 / copy_clone 按值拷贝与 clone（直接覆盖 `by_value_locals` 修复点）/ generic_impl_where_arg 泛型 / enum_discriminant 枚举分派 / repr-c-struct 内存布局），仅存 **ir** 维度快照（单文件 ~615K、6 例共 ~3.7M），用于捕获 IR 级回归与非确定性。
  - harness 新增 `--probe` 便捷模式：`python3 scripts/diff_harness.py check --probe` 等价于 `check --manifest tests/snapshot-baseline-ir-probe.txt --dims ir`。
  - CI（`.github/workflows/ci.yml`）新增 `IR probe regression (diff harness)` 步骤。
  - 验证：run 基线连续两次 `check` 均 56/0/0/0；IR 探针连续两次 `check` 均 6/0/0/0，无 IR 非确定性。

## 实现纪要（C2 扩充·路线 A，2026-09-04 续）
- **AST/HIR 排除 std 前缀（按源文件过滤条目）**：原 AST/HIR 文本被 std 前缀主导（单程序 `ast` ~21 万行、`hir` ~3.5 万行），逐程序存储不可行。路线 A 落地「用户代码过滤」：
  - `ast-user`：driver `emit_ast_user` 仅解析**用户源码**（`module::load_combined_source` 不含 std 预置），产物仅含用户顶层项。`hello` 实测 `ast` 212230 行 → `ast-user` 70 行。
  - `hir-user`：driver `emit_hir_user` 完整类型检查后，按 `HirItem.span` 字节偏移过滤掉落在 `prelude_len`（std 预置长度）之前的项。为此给 `rlyeh_hir::HirItem` 新增 `span: Span` 字段，并在 typecheck 全部 12 个 `HirItem` 构造点填入真实 decl span（用户项）或 `DUMMY_SPAN`（编译器生成项 / 单态化实例，无真实源位置 → 被排除）。`hello` 实测 `hir` 35354 行 → `hir-user` 36 行。注：泛型/闭包程序的 `hir-user` 仅含其非泛型用户项（实例为生成项）。
  - `rlyeh-driver` 新增 `EmitTarget::AstUser/HirUser`（`--emit ast-user/hir-user`）与库函数 `emit_ast_user`/`emit_hir_user`；`rlyeh-hir` 新增对 `rlyeh-lexer` 的依赖。
  - harness `DIMS` 增加 `ast-user`/`hir-user`（capture 自动路由到 `build --emit <dim>`）。
- **新增 AST/HIR 探针基线**：`tests/snapshot-baseline-ast-probe.txt`（13 例）、`tests/snapshot-baseline-hir-probe.txt`（12 例），仅存 `ast-user`/`hir-user` 维度快照（单文件数十~千余行）。
- CI 新增 `AST-user probe regression` / `HIR-user probe regression` 两步。
- 验证：ast-user 连续两次 `check` 均 13/0/0/0；hir-user 连续两次 `check` 均 12/0/0/0，无 AST/HIR 级非确定性。

## 风险分解（→ 中/低危）
- **K-M1（中）** 引导器（Rust driver）编译 Rlyeh 版**单组件**（如 `lexer`），经差分 harness 对拍。
- **K-M2（中）** 扩展为**双组件**（lexer + parser），验证组件间接口在 Rlyeh 侧一致。
- **K-M3（高→中）** 三阶段 bootstrap 校验：`rlyeh₂` ≡ `rlyeh₃`（字节/行为一致）。
- **K-M4（中）** 差分测试 harness 完善：IR 文本 / 行为 / 诊断三维比对，覆盖编译器多阶段产物。
- **K-M5（低）** 快照测试：AST/HIR/MIR/LIR 序列化快照回归比对。
- **L1（低）** 0.2.0 内先落地 harness 与 PoC 级对拍（lexer/parser），逐步扩展至全编译器。

## 受影响组件
`rlyeh-driver`（bootstrap 入口）、全部编译器 crate（被测对象）、`tests/`（自举/差分用例）。

## 验证
- 三阶段 bootstrap 产出 `rlyeh₂` 与 `rlyeh₃` 字节/行为一致。
- 差分 harness 对拍 Rust 参考与 Rlyeh 编译器产物（IR/行为/诊断）一致。

## 状态
🟢 0.2.0 PoC 三 checkpoint 全部落地：C0（driver `--emit` IR/AST/HIR 导出）✅ + C1（对拍 harness 脚手架）✅ + C2（运行行为快照基线 + CI 回归）✅（阶段 K）。AST/HIR/IR 体积约束已记录，留作未来双编译器差分维度。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 新增（评审发现：原评估遗漏分阶段自举 + 差分测试基础设施） |
| 2026-09-04 | 细化 0.2.0 收敛范围（harness + 单编译器快照，含 AST/HIR/IR 三维）；C0 落地 driver `--emit <ir\|ast\|hir>`；C1 落地 `scripts/diff_harness.py` 对拍 harness 脚手架（含双编译器预留），烟测通过 |
| 2026-09-04 | 修复 IR 非确定性根因：`by_value_locals` 内层 `HashSet`→`BTreeSet`（+ `propagate_by_value_aliases` 形参），发射顺序稳定；两轮 `--emit ir` 逐字节一致，harness 改为精确比对 |
| 2026-09-04 | C2 落地：harness 加 `--dims`/`--manifest`；新增 `tests/snapshot-baseline.txt`（精选 ~30 确定性用例）+ `tests/snapshots/` 运行行为基线（120K）；CI 加快照回归步骤；记录 AST/HIR/IR 被 std 前缀主导的体积约束 |
| 2026-09-04 | C2 扩充：run 基线用例 30→56（新增 26 个经双次运行验证的确定性用例）；新增 IR 探针 `tests/snapshot-baseline-ir-probe.txt`（小子集 6 例，仅存 ir 维度）+ harness `--probe` 便捷模式 + CI `IR probe regression` 步骤；验证 run 56/0/0/0、IR 探针 6/0/0/0 连续两次零差异 |
| 2026-09-04 | C2 扩充·路线 A：AST/HIR 排除 std 前缀（按源文件过滤条目）。`rlyeh_hir::HirItem` 新增 `span`，typecheck 12 个构造点填真实/合成 span；driver 新增 `--emit ast-user/hir-user`（`emit_ast_user`/`emit_hir_user`）。`hello` 实测 `ast` 212230→`ast-user` 70 行、`hir` 35354→`hir-user` 36 行。新增 AST/HIR 探针清单（13/12 例）+ harness `DIMS` 扩充 + CI 两步回归；验证 ast-user 13/0/0/0、hir-user 12/0/0/0 连续两次零差异 |