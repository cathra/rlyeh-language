# RFC：严格借用检查专项 — 引用生命周期 / Region 有效性（borrowck 生命周期检查）

| 字段 | 内容 |
|------|------|
| 状态 | Draft（规划中；T-0 已落地 2026-09-19；本文档为 borrow-simplification.md §7.5 将 B-5/B-7 移交而来的承载专项） |
| 日期 | 2026-09-19 |
| 范围 | 编译器前端：解析器（生命周期名贯通）→ 类型检查（Ref 携带 region 名）→ 借用检查 / region 检查（引用有效性求解与诊断）。**不改变** L0 安全语义（别名 XOR 可变、引用有效性）。 |
| 关联文档 | [borrow-simplification.md](./borrow-simplification.md)（B-5/B-7 移交来源）、[P012_L0借用检查器实现](../design/prompts/P012_L0借用检查器实现.md)、[g4-lifetime.md](../tasks/leaf/g4-lifetime.md)（现状：生命周期名解析后丢弃）、[04_所有权与借用检查器.md](../design/04_所有权与借用检查器.md)（已归档设计稿）、[semantics.md](../semantics.md)、[memory-model.md](../memory-model.md) |
| 任务拆解 | 见 §6 |

---

## 1. 背景与现状基线

借用检查生态当前由三个 crate 构成，能力递进：

1. **`crates/rlyeh-borrowck`**（P012，✅ 2026-08-20，但实际能力已超文档）：
   - `checker.rs` 已含 `BorrowKind::Shared/Mut` 排他 + **NLL 近似活跃期**（`born <= pos <= last_use`）；
   - 已有 `check_dangling_return` / `check_dangling_strfat_block`（悬垂引用检测，对应 Rust E0597 的 transfer/区域块变体）；
   - 但**仅服务 `transfer` / `&self` 接收者**，对一般 `&x`/`&mut x` 的 region 有效性**不求解**。
2. **`crates/rlyeh-regionck`**（独立 crate，测试 `regionck_test.rs` / `transfer_test.rs` 就绪）：
   - 负责 `region 'r {}` / `in 'r` / `transfer` 的区域约束；**处理的是「值跨区域转移」而非「引用存活」**。
3. **解析 / 类型层**（缺口核心）：
   - `g4-lifetime.md` 确认：`parse_type` 的 `&` 分支遇 `&'a T` 时**跳过生命周期名直接丢弃**；`parse_generics` 遇 `<'a>` 同理丢弃。
   - typecheck `resolve_ast_type` 的 `AstType::Ref` → `Type::Ref(inner)`，**不携带任何 region / 生命名**（grep 证实 `context.rs` 无 Ref 的 region 落点）。
   - borrow-simplification 的 B-4 已让 `struct Foo 'a { x: &'a T }` 的 `'a` 后缀进入 `AstStructDecl.region_param`，但**该字段当前未被任何约束求解消费**。

**结论**：`&mut` 排他（别名 XOR 可变）基线已具备；缺的是**引用的 region/生命周期良构性与推断诊断**——这正是 borrow-simplification 的 B-5、**B-7 所依赖、却始终停留在「规划中」的能力**。

---

## 2. 目标

- G1：**生命周期名贯通**——`&'a T` / `struct Foo 'a` 的生命名从解析一路携带到 HIR/类型，不再解析即丢弃。
- G2：**引用 region 有效性求解**——对 `&T` / `&mut T` 绑定，依据其所在 region / caller region / 显式 `'r`，判定引用是否在其指向值的生命周期内（E0597 Dangling 的 region 泛化版）。
- G3：**region 推断诊断（B-5）**——当 region 无法推断或引用逃逸出 referent region 时，给出精确错误（而非当前的「无提示 / 宽松放行」）。
- G4（可选，B-7）：**块级借用语义**——将借用活跃期从 NLL 近似（`born..last_use`）改为「最近 region / 词法块边界」区间模型，换取规则可人工推演、报错指向明确。

## 3. 非目标

- 不重写 regionck 的 `transfer` / `in 'r` 既有语义（已落地，仅新增「引用存活」维度）。
- 不引入数据竞争 / 悬垂（仅把既有宽松放行收紧为诊断）。
- 不删除 `*` / `&` / `&mut`（承重墙，见 borrow-simplification §1）。

---

## 4. 设计

### 4.1 生命周期名贯通（前置，B-5 必需）

- `AstType::Ref` 新增 `lifetime: Option<String>` 字段（`None` 表示省略，走默认 region 推断）。
- `parse_type` 的 `&` 分支：消费可选 `'a` 后**保留名**写入 `Ref.lifetime`（不再丢弃）；`<'a>` 泛型参数仍按既有 G4 行为丢弃（向后兼容零回归）。
- typecheck `resolve_ast_type` 的 `Ref` 分支产出 `Type::Ref(inner, lifetime)`（需要 `Type` 枚举新增二参，或新增 `Type::Ref(Box<Type>, Option<String>)`）。
- HIR 表达式 `AddrOf` / `AddrOfMut` 携带 `lifetime`（供 borrowck 区间计算）。
- 落点：`crates/rlyeh-ast`、`crates/rlyeh-parser/src/ty.rs`、`crates/rlyeh-typecheck/src/context.rs`、`crates/rlyeh-hir`（若需）、`crates/rlyeh-desugar`（构造点同步）。

### 4.2 Region 约束模型

- **caller region（默认）**：调用表达式所在 region / 词法块作为 `&T` 返回与 `&T` 绑定的默认 `'ret` 来源（borrow-simplification P1 已描述，本轮落地求解）。
- **显式 `'r` 绑定**：`struct Foo 'a` 的 `'a`、`fn f(x: &'r str) -> &'r str` 的 `'r` 进入约束变量集合。
- **约束生成**：对每个 `let r = &x;`、`&x` 实参、`return &x`，生成 `outlives(r.region, x.region)` 约束。
- 约束求解复用 / 扩展 regionck：regionck 现有区域约束求解器可承载 `outlives` 关系图，本轮增量接入「引用存活」边。

### 4.3 求解与诊断（G2/G3 = B-5）

- 在 typecheck 后、borrowck/regionck 阶段，对每条 `&T` 约束做良构性检查：
  - 引用 region ∉ referent region 的超集 → **DanglingReference**（E0597 泛化）。
  - region 无法从上下文推断（缺 caller region / 多 region 歧义）→ **RegionInferenceError**（B-5 诊断，取代当前「静默放行」）。
- 诊断格式对齐既有错误（`line:col: error[E0597]: ...` + `= help: ...`），复用 `rlyeh-driver` 的带偏移打印。

### 4.4 B-7 块级借用（可选，高风险）

- 将 borrowck 的 `Borrow` 活跃期模型从 NLL 近似（`born..last_use`）改为**块级区间**：活跃期 = 最近 `region 'r {}` 或词法块边界；`drop` / 重借可缩短。
- 需与 §4.2 的 region 约束模型协同（块 ≈ region），避免两套生命周期模型冲突。
- 代价：少数细粒度 NLL 模式需手动包 `{}`；建议作为**独立后续阶段**，不在本专项首轮切片。

---

## 5. 影响面与回归面

- **影响面**：解析器（Ref 保留名）、typecheck（`Type::Ref` 二参、约束生成）、borrowck/regionck（存活边 + 诊断）。diagnostics 文案。
- **回归面**：中。`740+` 现有用例作为守护；重点回归含 `&T` 返回 / struct 含 `&T` 字段 / `&mut` 排他的用例。需全量 `cargo test --workspace` + `cargo clippy --workspace --all-targets` 0 警告准入。

---

## 6. 任务拆解

| ID | 任务 | 对应 | 优先级 | 风险 |
|----|------|------|--------|------|
| T-0 | `AstType::Ref` + `Type::Ref` 携带 `lifetime: Option<String>`；parser 保留 `'a` | 前置 | 中 | ✅ 已落地（2026-09-19） |
| T-1 | parser `&'a T` 保留生命周期名并经 `resolve_ast_type` 透传至 `Type::Ref` 第三字段（HIR 类型由 `Type` 派生，无需独立 `lifetime` 字段） | 前置 | 中 | ✅ 已落地（2026-09-19） |
| T-2 | regionck 扩展 `outlives` 约束图，接入「引用存活」边（落地于 borrowck：引用存活边经拷贝 / 块值传播闭合别名逃逸缺口） | G2 | 中 | ✅ 已落地（2026-09-19） |
| T-3 | 引用 region 良构性检查 + **DanglingReference** 诊断（region 边界维度：区内局部被引用、region 退出后引用仍存活 / 区尾引用逃逸到区外变量） | G2/G3 | 中 | ✅ 已落地（2026-09-19） |
| T-4 | **region 推断失败诊断**（取代静默放行） | **B-5** | 中 | ✅ 已收口（2026-09-19）：多 region 歧义被 Rlyeh 生命周期省略覆盖，局部逃逸由 T-2/T-3 `DanglingReference` 兜底，无新增诊断（见 §9.7 方案 A） |
| T-5 | run-pass / compile-fail 测试：`lifetime_region_valid`（新增）/ `dangling_region_err`（已由 T-3 `dangling-region-final`/`dangling-region-assign` 覆盖）/ `region_inference_err`（随 T-4 方案 A 取消） | G3/B-5 | 低 | ✅ 已落地（2026-09-19） |
| T-6（可选） | **块级借用语义**：borrowck 活跃期改为块级区间 | **B-7** | 高 | 高 |

建议首轮切片：**T-0 → T-1 → T-2 → T-3 → T-4 → T-5**（B-5 收口）；**T-6（B-7）独立评估**。

---

## 7. 验收标准

- `&'a T` / `struct Foo 'a` 的生命名从解析贯通到 HIR/类型，不再丢失。
- 引用逃逸出 referent region 时给出 `DanglingReference` 错误（取代当前宽松放行）。
- region 无法推断时报精确诊断（B-5）。
- 全量测试套件零回归；`cargo clippy --workspace --all-targets` 0 警告。
- （B-7 若启动）借用活跃期 = 块级区间，既有 NLL 近似合法用例保持通过或按文档迁移。

## 8. 开放问题

1. `Type::Ref` 由单参改二参是否破坏既有 `Type` 模式匹配（desugar/codegen/MIR）？需全量 grep 确认落点数量（建议 T-0 首步做影响面清点）。
2. regionck 现有求解器能否直接承载 `outlives` 图，还是需新增轻量求解通道？
3. B-7 块级模型与 §4.2 region 约束模型的协同边界（块 ≈ region 是否 1:1）？
4. 默认 caller region 与显式 `'r` 混用时的约束优先级与诊断措辞？

---

## 9. T-0 影响面清点（2026-09-19）

为落地 T-0（`AstType::Ref` / `Type::Ref` 携带 `lifetime`），先清点全仓改造落点，作为 B-5 动手前的必要前置。

### 9.1 枚举定义（2 处，改造起点）

- `crates/rlyeh-ast/src/lib.rs:923`：`AstType::Ref(Box<AstType>, bool)` → 加第三字段 `Option<String>`（或独立 `Lifetime` 类型）。
- `crates/rlyeh-typecheck/src/types.rs:68`：`Type::Ref(Box<Type>, Mutability)` → 加第三字段 `Option<String>`。

### 9.2 构造点（需补第三实参，默认 `None`）

- `Type::Ref(Box::new(..), m)` 约 **12** 处：`check_item/collect.rs`、`check_stmt.rs`、`check_expr/{resolve,builtin,method,index_enum,call,mod,misc}.rs`、`util.rs`（`substitute` / `type_to_ast` 递归重构，保持原 `m`）。
- `AstType::Ref(Box::new(..), is_mut)` 约 **8** 处：`rlyeh-desugar/src/generate/mod.rs`（2：`Self` / `Context` 接收者）、`rlyeh-parser/src/ty.rs`（1：解析入口）、`check_item/derive.rs`（4：derive 生成）、`check_expr/util.rs`（1：`type_to_ast` 递归）。

### 9.3 匹配点（需补第三绑定 `_`）

- `Type::Ref(inner, _)` / `(a, ma)` 等模式约 **16** 处，分布于 typecheck 各 `check_*` 模块（`types.rs` / `comparison.rs` / `fn_sig.rs` / `mod.rs` / `collect.rs` / `derive.rs` / `check_stmt.rs` / `builtin.rs` / `thread.rs` / `method.rs` / `resolve.rs` / `generic.rs` / `util.rs` / `index_enum.rs` / `call.rs` / `mod.rs` / `json.rs` / `misc.rs` / `ctrl.rs` / `field.rs` / `in_expr.rs` 等）。
- `AstType::Ref(inner, _)` 模式 **3** 处：`rlyeh-parser/src/item.rs`、`tests/control.rs`、`tests/parser_test.rs`。

### 9.4 下游零引用（关键，正面回答 §8 Q1）

在 `rlyeh-hir` / `rlyeh-mir` / `rlyeh-codegen` / `rlyeh-lir` / `rlyeh-llvm*` / `rlyeh-driver` / `rlyeh-borrowck` / `rlyeh-regionck` 中检索 `Type::Ref` 命中 **0**。

**结论**：`Type` 枚举的「二参 → 三参」改造爆炸半径覆盖 `rlyeh-typecheck` + `rlyeh-parser` + `rlyeh-desugar` + `tools/rlyeh-doc` + `tools/rlyeh-fmt`（含 parser 测试与文档/格式化工具）；HIR / MIR / codegen / LIR / LLVM 后端 / driver / borrowck / regionck 中 `Type::Ref` 命中 **0**，使用独立类型表示，无需改动。这把 T-0 风险从「全编译器」降为「前端三 crate + 两工具」机械改参（实际落地 84 + 5 处，于 2026-09-19 完成）。

### 9.5 判定

T-0 已落地（2026-09-19）：约 **40** 处落点（2 定义 + 20 构造 + 18 匹配）均为「追加 `None` / 第三绑定 `_`」，借助编译器报错逐项消歧完成；parser / desugar 的接收者构造（`Self` / `Context`、`&self`）默认 `None`。已作为 B-5 首步**独立提交**，不混入其它逻辑。

**T-1 已落地（2026-09-19）**：parser `&` 分支此前 `MVP 解析后丢弃` 生命周期名，现已通过 `expect_lifetime` 捕获 `'a` 标签并写入 `AstType::Ref` 第三字段；typecheck `resolve_ast_type` 的 Ref 分支将该字段 clone 透传至 `Type::Ref` 第三字段（`#[memory(gc)]` 模块的 `Gc<T>` 路径忽略之）。新增 parser 单测 `test_ref_lifetime_label_retained`（`&'a T` / `&T` / `&'b mut T` 三态）守护。全仓编译 + parser/typecheck 单测 + driver 全量集成套件（740+ 用例）均零回归。

**T-2 已落地（2026-09-19）**：引用存活边经拷贝 / 块值传播，闭合别名逃逸缺口。原 `check_dangling_return` 仅覆盖 `return` 与函数体块尾值，对 `let s = r;` / `s = r` / `let s = { ...; r }` 这类「引用经变量拷贝逃逸」的路径静默放行。本轮在 `crates/rlyeh-borrowck/src/checker.rs` 新增 `ref_source_var`（抽取初始化 / 赋值表达式最终求值的引用变量，含块尾值）与 `copy_borrow`（为拷贝目标登记同源借用，使引用存活边随别名传播）；并修复 `is_escaping_root` 的时序缺陷——原实现在查错时刻重算被引用变量是否逃逸，但内层作用域已出栈会误判为否，故改为在借用创建时刻快照 `escapes` 标志存于 `Borrow` 结构，查错时直接采用快照。新增 compile-fail 用例 `dangling-ref-alias.rl` 守护；driver 全量集成套件（740+ 用例）零回归。注：RFC 原规划 T-2 落点为 regionck `outlives` 约束图；因 DanglingReference 诊断实际栖身 borrowck（且 regionck 当前不追踪引用），本轮在 borrowck 内等价落地「引用存活」边，未改动 regionck 既有 `transfer` / `in 'r` 语义（与 §3 非目标一致）。

**T-3 已落地（2026-09-19）**：引用 region 良构性检查的 region 边界维度。此前悬垂检查仅覆盖「引用逃逸出**函数帧**」（`return` / 函数体块尾值 / 引用别名，T-2）；T-3 新增「引用指向 `region 'r {}` 内创建的值、region 退出时批量释放后该引用仍存活」的 `DanglingReference` 诊断。落点 `crates/rlyeh-borrowck/src/checker.rs`：
- 新增 `region_local_stack: Vec<Vec<String>>`（与词法 `scopes` 平行），在 `check_stmt` 的 `let` 落点将区内声明的局部名登记为「region 局部」（参数 / 全局 / region 外绑定不入册）；`recent_region_locals` 暂存 region 退出时的局部名供 `let r = region 'r { &x }` 落点复用。
- region 退出扫描：遍历借用，凡「生于区内（`start < born <= boundary`）、指向 region 局部、且 `last_use > boundary`（活到区外）」者报 `DanglingReference`；覆盖 `r = &x`（区内赋值到区外变量）等路径。
- `check_stmt` 分流 `let r = region 'r { &x }`：区尾引用指向 region 局部即直接报悬垂（值已逃逸出 region），不重复登记具名借用以免后续 `return r` 叠加报错。
- `check_expr` 的 `Assign` 臂对 `r = &x` 直接为 `r` 登记以 `x` 为源的具名借用，使 region 边界扫描能捕获引用随 `r` 逃逸的路径。
新增 compile-fail 用例 `dangling-region-final.rl` / `dangling-region-assign.rl` 守护；driver 全量集成套件（740+ 用例）零回归。

**已知限制（T-3，与 T-2 别名限制同构）**：`let r = region 'r { s }`（`s` 为持有区内部 `&x` 的引用变量）这类「region 块尾值是引用变量别名」的路径，因 `ref_source_var` 仅处理 `Block`/`UnsafeBlock` 的块尾变量、不处理 `Region` 块尾别名，本轮未捕获（需将 `Region` 纳入 `ref_source_var` 的块尾值抽取，留待后续）。其余「区尾直接 `&x`」与「区内 `r = &x` 赋值逃逸」两类均已覆盖。

## 9.7 T-4 阻塞说明：与生命周期省略语义冲突（2026-09-19）

T-4 原文「多 region 歧义 → `RegionInferenceError`（B-5 诊断，取代静默放行）」在 Rlyeh 当前语义下**不可直接落地**，原因：

- Rlyeh 已采纳**生命周期省略（lifetime elision）**：`fn longest(x: &i64, y: &i64) -> &i64 { if *x > *y { x } else { y } }` 这类「返回引用可能来自任一 `&` 参数」是**合法且预期**的模式（与 Rust 单输入生命周期省略同构）。
- 既有 `tests/run-pass/lifetime.rl`、`tests/run-pass/lifetime_omit.rl`、`tests/run-pass/borrow_pass.rl` 三个 run-pass 用例**断言该模式编译运行成功**，注释明确「省略 'a 默认可用（默认 caller region）」「参数引用返回合法（生命周期 elision）」。
- 因此若在 borrowck 中对「返回引用源自多个互不相容输入区域」报错，将**破坏这 3 个 run-pass 用例**——违背了 borrowck 专项「不改变 L0 安全语义」的非目标。

**结论与待决策**：T-4 的「多 region 歧义」目标已被 Rlyeh 的 elision 语义覆盖（返回引用来自参数即合法，来自局部则由 T-2/T-3 的 `DanglingReference` 兜底）。真正「region 无法推断」的硬性错误场景在当前模型中**不存在可触发的静默放行缺口**（局部逃逸已查、参数来源合法、全局来源免检）。

需从以下二者择一后方可继续 T-4：
- **方案 A（推荐，零语义变更）**：认定 T-4 已由「elision + DanglingReference（T-2/T-3）」实质收口，关闭 T-4，不再新增 `RegionInferenceError` 诊断；后续若有显式生命周期标注（`fn f<'a>(x: &'a T, y: &'a T) -> &'a T`）的*不一致*场景，再作为独立任务评估（需先打通 HIR 签名生命周期贯通，见 §8 Q1/§4.1）。
- **方案 B（收紧语义，语言级决策）**：改为要求多 `&` 参数返回引用时**必须显式标注生命周期**，将 `lifetime.rl`/`lifetime_omit.rl`/`borrow_pass.rl` 转为 compile-fail 并引入 `RegionInferenceError`。此方案改变既有语言行为，属语义变更，须先于专项确认。

> 注：本专项 T-0~T-3 已落地并提交（commit `77746ae` T-2、`d236aff` T-3）；T-4 的 `RegionInferenceError` 变体与检测逻辑已原型验证会冲突，已回退，代码恢复至 T-3 状态，全量套件（740+ 用例）零回归。

**决策（2026-09-19，方案 A）**：经原型验证与 run-pass 用例核对，采纳**方案 A**——T-4 的「多 region 歧义 → `RegionInferenceError`」在 Rlyeh 当前语义下不成立（elision 已使该模式合法），故**关闭 T-4、不新增诊断**。原 RFC §4.3 中「多 region 歧义 → `RegionInferenceError`（B-5 取代静默放行）」的规划，其目标已被「elision（参数来源合法）+ `DanglingReference`（T-2/T-3，局部逃逸兜底）」实质覆盖，无独立诊断价值。后续若引入显式生命周期标注的*不一致*检查（如 `fn f<'a>(x: &'a T, y: &'a T) -> &'a T` 返回与 `'a` 不符的引用），须先打通 HIR 签名生命周期贯通（见 §8 Q1 / §4.1），另立任务评估。T-5 的 `region_inference_err` 测试随 T-4 取消；`dangling_region_err` 已由 T-3 的 `dangling-region-final.rl` / `dangling-region-assign.rl` 覆盖；新增 run-pass `lifetime_region_valid.rl` 守护「合法 region 借用不被误拒」。

## 9.8 已知缺陷（2026-09-19）：`uses` 按名索引、不区分同名遮蔽

T-5 编写 `lifetime_region_valid.rl` 时暴露：**borrowck 的 `uses: HashMap<String, Vec<usize>>` 以变量名为键，不区分同一函数内同名遮蔽（shadowing）的多个绑定**。`register_borrow` / `copy_borrow` 据此计算 `Borrow.last_use`，`active_borrows`（NLL 活跃判定）与 T-3 的 region 边界扫描（`last_use > boundary`）均依赖该值。

- **触发**：同一函数内两个不同作用域各自 `let r = &x;`（同名 `r` 遮蔽），前者（`born` 较小）的 `last_use` 会拾取后者 `*r` 的使用位置（更大 pos），若后者位于 region 之后，则前者被判 `last_use > boundary` → 误报 `DanglingReference`。
- **性质**：**既有缺陷，非 T-3 引入**。T-3 的 region 边界扫描是首个在「跨 region 比较 `last_use` 与 `boundary`」处暴露该缺陷，但根因在 `uses` 的按名索引模型。
- **规避**：T-5 测试改用互不相同的变量名（`x1/r1`、`z2/rz2`、`a3/r3` 等）以不触发该缺陷；合法代码亦可如此规避。
- **修复（2026-09-19，已落地）**：`BorrowChecker` 新增 `defs: HashMap<String, Vec<usize>>`（预扫描记录各次 `let` 定义位置），并新增 `last_use_for(var, born)`——仅取「`>= born` 且 `< 下一次同名重定义`」区间内 `uses[var]` 的最大值，应用于 `register_borrow` / `copy_borrow` 计算 `Borrow.last_use`。无遮蔽时 `next_def = ∞`，等价于原 `uses[var].last()`（**零行为变化**）；遮蔽时各绑定实例活跃期互不干扰，`last_use` 不再被后续同名绑定的使用位置污染。新增 run-pass `lifetime_region_shadow.rl`（复用同名引用变量跨 region）锁定该修复。全量套件（740+ 用例）零回归。
