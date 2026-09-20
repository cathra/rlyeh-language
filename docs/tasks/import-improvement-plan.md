# import 完善策划（P1 `pub module` / 诊断健壮性 / P2 可见性 / P4 dagon 包集成）

> **状态**：✅ 已收口（2026-09-20）· **范围**：用户确认「前面 4 项都要」——即补齐 `docs/module-system.md` 路线图中尚未落地的 import 相关四块能力。
> **基线**：截至 2026-09-19，import 已实现单路径+别名、`pub import` 重导出、组导入（含嵌套子组）、glob、`pub use` 多级链、跨模块路径访问；全量套件约 249 用例 0 失败（见 `leaf/sh-p1-3-nested-module.md`）。
> **原则**：不破坏现有语法与存量代码（`module`/`import` 关键字 2026-08-25 已更名迁移）；每一项独立可交付、可回滚；先评审后实现。

---

## 0. 背景与缺口总览

`docs/module-system.md` 当前 import 能力矩阵（✅=已实现）：

| 能力 | 状态 | 本策划对应 |
|------|------|-----------|
| `module`/`import` 关键字、`module name;` 外部文件、内联模块、多级外部模块 | ✅ | — |
| `import path;` + `as` 别名、跨模块路径访问 | ✅ | — |
| `import a::{b, c}` 组导入（含嵌套子组 `a::{b::{x,y}, c}`） | ✅（2026-09-02/04） | — |
| `import a::*` glob 导入 | ✅（2026-09-02） | — |
| `pub import` 重导出（多级链） | ✅（2026-09-02） | — |
| 模块内 `import` 相对本模块解析（2026-09-18 规则） | ✅ | — |
| **`pub module` 声明** | 📋 未实现 | **工作流 A** |
| **import 健壮性诊断**（冲突 / 歧义 / 候选） | 📋 未实现（§4.2/§4.3 规划） | **工作流 B** |
| **可见性检查**（默认私有 + `pub`） | 🔧 仅语法无校验（B4 暂缓） | **工作流 C** |
| **dagon 包集成**（`rlyeh build` 注入依赖模块） | 📋（P4） | **工作流 D** |

---

## 1. 总览表

| 工作流 | 目标 | 风险 | 主要改动组件 | 依赖 |
|--------|------|:----:|-------------|------|
| **A** `pub module` | 模块本身可 `pub` 对外暴露，被跨模块 `import` | 低 | parser（`parse_item`/`parse_mod`/`AstModDecl`）、typecheck `collect`、driver `module.rs` | 无 |
| **B** 诊断健壮性 | 别名冲突 / glob 歧义 / `NameNotFound` 候选 | 低 | typecheck `register_use`/`resolve_full_name`/`error.rs`/`context.rs` | 无 |
| **C** 可见性（P2） | 默认私有 + `pub` 两级校验，跨模块访问私有报错 | 高（std 迁移） | typecheck 可见性表 + 检查器、`pub` 标注、std 迁移 | 建议 A 先于 C（模块对外面先成型） |
| **D** dagon 包集成（P4） | `rlyeh build --dep-root` 注入第三方依赖模块名空间 | 中 | driver CLI + 模块载入、dagon `lib.rs`/`build.rs` | C 的 `pub` 作为依赖暴露契约；建议 C 后做 |

> **建议顺序**：A → B → C → D。A/B 低风险且独立，快速见效；C 风险最高需配套 std 迁移与过渡策略；D 以 C 的 `pub` 暴露契约为依赖，放最后。

---

## 2. 工作流 A：`pub module` 声明

### 目标
支持 `pub module foo;`（外部文件）与 `pub module foo { ... }`（内联），与既有 `pub import` 对齐——让模块本身成为「对外公共面」的一部分，可被**外部 crate / 顶层导入者**经 `import parent::foo::item` 访问。非 `pub` 的嵌套模块仅在同一 crate 模块树内部可达（同模块及其子模块可直接引用），不对外部导入者暴露。

### 改动点
1. **AST**（`crates/rlyeh-ast/src/lib.rs`，`AstModDecl` ~line 261）：新增字段 `pub is_pub: bool`。
2. **parser 分发**（`crates/rlyeh-parser/src/parser.rs` ~line 202）：`Some(Token::Mod)` 分支先尝试 `eat(Pub)` 得到 `is_pub`，传给 `parse_mod`。
3. **parser `parse_mod`**（`item.rs` ~line 619）：签名增加 `is_pub: bool` 参数，构造 `AstModDecl` 时填入。
4. **typecheck 模块收集**（`crates/rlyeh-typecheck/src/check_item/collect.rs`）：递归收集模块时，将 `is_pub` 模块的前缀登记进「对外公共模块面」（新增 `pub_module_prefixes: HashSet<String>` 于 `Context`）；`resolve_full_name` / `resolve_callable` 在跨模块（调用方前缀非 `m` 或其后代）引用 `m::item` 时，要求 `m` 在 `pub_module_prefixes` 中，否则报 `PrivateModule`。
5. **driver 模块展开**（`crates/rlyeh-driver/src/module.rs` ~line 74）：外部文件形式 `pub module foo;` 路径解析（`foo.rl`/`foo/module.rl`）不变，仅携带 `is_pub` 透传到 typecheck。

### 语义决策点（待评审）
- **C1**：非 `pub` 嵌套模块的可见范围——草案定义为「仅同一 crate 模块树内部可达（父/兄弟/子模块可直接引用，外部导入者不可达）」。需确认是否与「扁平名字空间、全部可达」既有行为冲突（既有非 `pub` 模块当前实际全部可达）。若担心回归，可折中为：A 阶段先只让 `pub module` **额外开放**对外可达性，不改变非 `pub` 模块的既有全可达行为（即 A 仅为「增量放开」而非「收紧」），真正的收紧留给工作流 C 统一处理。
- **推荐**：A 采用「增量放开」语义（C1 折中），避免与 C 的可见性收紧耦合，降低回归风险。

### 验收
- run-pass：`tests/run-pass/pub_module_export.rl`（顶层 `pub module inner { pub fn f() }`，外部 `import inner::f` 调用）→ 通过。
- compile-fail：`tests/compile-fail/private_module_import.rl`（非 `pub` 模块被外部 `import` → 报 `PrivateModule`）——若采用「增量放开」语义则本用例延后到 C。
- 全量套件回归 0 失败。

---

## 3. 工作流 B：import 健壮性诊断

### 目标
落地 `docs/module-system.md` §4.2/§4.3 规划的错误诊断，消除「静默遮蔽 / 报错无信息」：
- **别名冲突 `NameConflict`**：`import a::x as y;` 中 `y` 已作为本地符号或其它 `import` 别名存在 → 报错而非静默遮蔽。
- **glob 歧义**：`import a::*; import b::*;` 二者均导出 `x`，使用处 `x` 报歧义（提示显式化路径）。
- **`NameNotFound` 候选**：`resolve_full_name` 失败时，给出拼写相近的候选符号 + 「依赖未声明」提示。

### 改动点
1. **`register_use`**（`check_item/mod.rs` ~line 199）：插入 `use_aliases` / 局部作用域前检测 `alias` 或 group 叶子名是否已存在 → 报 `NameConflict`（带冲突双方 span）。
2. **glob 登记**（`register_use` glob 分支 ~line 231）：维护「glob 导出符号 → 来源模块」多对一映射；解析使用处名称时，若命中 ≥2 个 glob 来源 → 报 `GlobAmbiguity`。
3. **`resolve_full_name`**（`context.rs` ~line 414）：未命中时，计算所有已知符号（structs/fn_signatures/consts/actors/use_aliases + 模块前缀）的编辑距离，取 top-3 相近名附到 `NameNotFound`。
4. **错误类型**（`crates/rlyeh-typecheck/src/error.rs`）：新增 `NameConflict` / `GlobAmbiguity` / `PrivateModule`（C 用）；`NameNotFound` 增加 `candidates: Vec<String>` 字段。
5. **显式优先规则**：显式 `import` 符号与 glob 同名时显式优先（glob 不遮蔽），已在 §4.3 约定，本工作流落实现。

### 验收
- run-pass：`tests/run-pass/import_explicit_shadows_glob.rl`（`import a::x; import a::*;` 中 `x` 用显式）→ 通过。
- compile-fail：`tests/compile-fail/import_alias_conflict.rl`（`import a::x as y; let y = 1;` → `NameConflict`）、`import_glob_ambiguity.rl`（两 glob 同名 → `GlobAmbiguity`）、`import_not_found.rl`（`import nonexist::x;` → `NameNotFound` 带候选）。
- 全量套件回归 0 失败。

---

## 4. 工作流 C：可见性检查（P2）

### 目标
实现「默认私有 + `pub` 两档」可见性：符号（fn/struct/enum/protocol/const/static/module/import）仅当 `pub` 或被访问方位于其模块树内部时，可被跨模块访问；私有符号被外部模块引用时报 `PrivateItem`。仅两档，不支持 `pub(crate)`/`pub(super)`（扁平名字空间下无此概念）。

### 设计
- **可见性表**（新增于 `Context`）：`pub_symbols: HashSet<String>`（全名 `m::item`，`is_pub` 项收集阶段登记）。
- **访问判定**（在 `resolve_full_name` / `resolve_callable` / `resolve_ast_type` / `resolve_named_type` 解析完成后）：若调用方模块前缀 `caller` 与定义方前缀 `def` 满足 `caller == def || caller.starts_with(&format!("{def}::"))`（即同模块或 `def` 的后代）则放行；否则要求 `def::item ∈ pub_symbols`，否则报 `PrivateItem`。
- **`is_pub` 落地情况**：`AstFnDecl` / `AstModDecl` 及 `import` 此前已有 `is_pub`；本次（2026-09-20）补齐 `AstStructDecl` / `AstEnumDecl` / `AstTraitDecl`（关键字 `protocol`，`trait` 已从语法移除）/ `AstActorDecl` 的 `is_pub` 字段，parser 在 `pub` 分支置 `true`、构造器默认 `false`（含 desugar 生成的 Future 结构体），收集阶段按前缀 `full_name(prefix, name)` 登记进 `pub_symbols`。

### 风险与迁移策略（关键）
文档 B4 明确指出：std 全线依赖「跨模块全部可达」，启用可见性校验需为 std 全部跨模块引用补齐 `pub`，风险高、易引发大范围回归。
- **过渡策略（推荐）**：引入 driver 编译开关 `--visibility=warn|error|off`（默认 `off`，保持现状）。
  - 阶段 1（本工作流）：实现校验逻辑 + `--visibility=warn` 仅告警不报错，跑全量 std/示例/测试，收集所有「本应私有却被跨模块访问」的点，批量补 `pub`。
  - 阶段 2：补完 `pub` 后切 `--visibility=error` 为默认，新增 `tests/compile-fail/private_item_access.rl` 锁定。
- 该开关保证：未就绪前存量代码零影响，可灰度推进。

### 验收
- compile-fail：`tests/compile-fail/private_item_access.rl`（模块内 `fn secret()` 非 `pub`，外部 `import m::secret` 调用 → `PrivateItem`）。
- 迁移后全量套件（含 std）在 `--visibility=error` 下 0 失败。

---

## 5. 工作流 D：dagon 包集成（P4）

### 目标
让 `rlyeh build` 接收 `--dep-root <pkg>=<dir>`（可多段），将第三方依赖的 `src/lib.rl`（或 `module.rl`）作为「虚拟模块集合」载入模块名空间；依赖模块经 `pub`（工作流 C 契约）暴露，主程序以 `import foo::bar;` 访问。dagon 已完成 resolve/lock（`Rlyeh.lock` 版本求解），本工作流补齐编译器侧注入。

### 改动点
1. **driver CLI**（`crates/rlyeh-driver/src/main.rs`）：`build` 子命令新增 `--dep-root <pkg>=<dir>` 重复参数解析。
2. **模块载入**（参考 `module.rs` 文本展开逻辑）：对每个 `--dep-root foo=/path/to/foo`，读取 `foo/src/lib.rl`（回退 `module.rl`），以 `foo` 为根前缀载入 `Context.module_prefixes` / `pub_module_prefixes`，与本地扁平名字空间合并；依赖间同名冲突按 `Rlyeh.lock` 版本锁定（dagon 已解，编译期无重复定义）。
3. **std 不变量**：标准库仍走 `RLYEH_STD_PATH`，不进入依赖图（与现有机制一致）。
4. **dagon 对接**：复用 `dagon/src/lib.rs` 公开的 `resolve`/`manifest` API 读取 `Rlyeh.toml`/`Rlyeh.lock`，拿到依赖根列表后逐一注入 driver。

### 验收
- 端到端：`examples/` 或 `tests/` 下新建含 `Rlyeh.toml` + 本地 `--dep-root` 指向的示例库项目，`rlyeh build --dep-root demo=/tmp/demo` 编译通过，`main.rl` 经 `import demo::api::foo` 调用并运行输出正确。
- 全量套件回归 0 失败（无 `--dep-root` 时行为不变）。

### 实施状态（2026-09-20）：D 已落地

`--dep-root pkg=dir` 端到端可用：`main.rs` 解析重复参数 → `CliOpts.dep_roots` →
`IncrementalDriver::with_dep_roots` → `compile_file_to_llvm` 经
`module::inject_dep_roots` 将依赖入口（`src/lib.rl`，回退 `lib.rl` / `module.rl`）以
`pub module pkg { ... }` 包裹前置。`import pkg::item;` 正确解析（依赖经工作流 C 的
`pub` 契约暴露公共面）。

**补充修复**：`IncrementalDriver::compile_to_llvm(file, source)`（字符串源码路径，
供 `run_source` 等使用）此前绕过 `inject_dep_roots`，导致 `dep_roots` 仅磁盘路径生效、
字符串路径被忽略；现统一在编译前注入。回归用例：
`crates/rlyeh-driver/tests/dep_root_test.rs`（`dep_root_injection_resolves_pub_module`
输出 49、`dep_root_missing_entry_errors` 缺失入口报错）。

---

### 实施状态（2026-09-20）：`pub` 覆盖 struct / enum / protocol / actor

工作流 A（`pub module`）与工作流 C 的 `--visibility` 开关、可见性表、检查器均已落地；本日补齐工作流 A 选择的「**`pub` 标注扩展到 struct / enum / protocol / actor**」（用户确认：trait 关键字已改名为 `protocol`，对应 AST 仍为 `AstTraitDecl`）。

**改动落点**
- `crates/rlyeh-ast/src/lib.rs`：`AstStructDecl` / `AstEnumDecl` / `AstTraitDecl` / `AstActorDecl` 新增 `pub is_pub: bool`。
- `crates/rlyeh-parser/src/item.rs`（`parse_struct`/`parse_enum`/`parse_trait`/`parse_actor` 构造器默认 `false`）、`actor.rs`、`crates/rlyeh-desugar/src/generate/mod.rs`（生成的 Future 结构体默认 `false`）。
- `crates/rlyeh-parser/src/parser.rs`：`parse_item` 的 `Some(Token::Struct|Enum|Protocol|Actor)` 分支在消费 `pub` 后置 `is_pub = true`。
- `crates/rlyeh-typecheck/src/check_item/mod.rs`：`collect_declarations` 对各声明分支，当 `is_pub` 为真时把 `full_name(prefix, name)` 登记进 `pub_symbols`（枚举额外登记全部变体 `Enum::Variant` 全名，继承 Rust 语义）。
- 可见性接线：`crates/rlyeh-typecheck/src/check_expr/construct.rs`（struct 字面量 + 枚举变体结构式构造）、`call.rs`（actor `X::new()`/`new_supervised` 分支）补充 `check_visibility`；类型名解析（`context.rs:resolve_named_type` 约 653 行）与函数/路径解析（`call.rs`/`mod.rs`）本已复用 `pub_symbols` 通用判定。

**验证（CLI：`--no-std --visibility=error`）**
- 正向：`pub struct`/`pub enum`/`pub protocol`/`pub actor` 跨模块引用 → 通过。
- 反向：私有 `struct`（`m::S`）、私有 `enum` 变体（`m::E::A`）、私有 `actor` 类型标注（`fn f(a: m::Act)`）→ 均报 `PrivateItem`（`m::S` / `m::E::A` / `m::Act`）。

**回归套件（2026-09-20）**：test harness（`crates/rlyeh-driver/src/test_runner.rs`）新增 `// flag:` 注释解析（`--visibility=error|warn|off` 与 `--no-std`，多段累加），使 compile-pass / compile-fail 用例可注入用例级 CLI 选项（此前全部走默认 `Off` + 注入 std，无法覆盖可见性）。新增用例：`tests/compile-fail/private-struct.rl`、`private-enum-variant.rl`、`private-actor-type.rl`、`private-actor-spawn.rl`（均 `--visibility=error --no-std`，断言 `m::S` / `m::E::A` / `m::Act` / `m::PrivateActor` 的 `PrivateItem`）、`tests/compile-pass/visibility-pub.rl`、`visibility-pub-actor-spawn.rl`（pub struct/enum/actor 跨模块引用与 spawn 应编译通过）。全量 `rlyeh test tests` 318 用例 0 失败。

**已知限制（架构特性，非本次 `pub` 扩展缺陷）**
1. ~~actor `X::new()` spawn 路径不接入可见性~~ **已闭合（2026-09-20）**：`check_expr/call.rs` 的 actor 构造分支（`Counter::new()`/`new_supervised`）在生成 `rlyeh_actor_spawn` 前已调用 `check_visibility(&actor_full)`，私有 actor 跨模块 spawn 在 `--visibility=error` 下正确报 `TC033`。回归用例：`tests/compile-fail/private-actor-spawn.rl`（拒绝）、`tests/compile-pass/visibility-pub-actor-spawn.rl`（pub 放行）。actor 的**类型名**引用（如 `fn f(a: m::Act)`）亦受校验（见 `private-actor-type.rl`）。
2. ~~protocol 跨模块约束引用语法受限~~ **已闭合（2026-09-20）**：`parse_conformance_list` 与泛型参数 bound 现经新增 `parse_qualified_name` 解析 `::` 限定名，故 `impl T: m::P` / `struct C: m::P` / `protocol A: m::P` / `fn f<T: m::P>()` 均已支持跨模块协议引用；下游 `resolve_trait_key` / `names_match` 原已能解析 `::` 限定 trait 名，无改动。回归用例：`tests/run-pass/protocol_cross_module.rl`（输出 7）、`tests/compile-pass/protocol-cross-module.rl`，及 parser 单测 `test_conformance_list_qualified_name` / `test_impl_conformance_qualified_name` / `test_generic_param_bound_qualified_name`。

## 6. 依赖与批次

```
A (pub module, 低) ──→ B (诊断, 低) ──→ C (可见性, 高, 需 --visibility 灰度) ──→ D (包集成, 中, 依赖 C 的 pub 契约)
        │                    │
        └────────────────────┴── A/B 独立可先落地，不阻塞彼此
```

| 批次 | 工作流 | 理由 |
|------|--------|------|
| 批次 1 | **A** `pub module` | 低风险，与 `pub import` 对齐，先成型「模块对外面」 |
| 批次 2 | **B** 诊断健壮性 | 低风险，纯错误诊断增强，零行为破坏 |
| 批次 3 | **C** 可见性 | 高风险，分阶段：`warn` 灰度收集 → 补 `pub` → `error` 默认 |
| 批次 4 | **D** dagon 包集成 | 中风险，依赖 C 的 `pub` 暴露契约，CLI 注入 |

---

## 7. 风险与缓解

| 风险 | 等级 | 缓解 |
|------|:----:|------|
| C 启用可见性致 std 大范围回归 | 高 | `--visibility` 三态开关，先 `warn` 收集再 `error`；std 批量补 `pub` |
| A 改变非 `pub` 模块既有「全部可达」行为 | 中 | A 采用「增量放开」语义，仅 `pub module` 额外开放，不收紧既有 |
| D 依赖图循环 / 同名冲突 | 中 | dagon `Rlyeh.lock` 已版本求解；driver 载入期 `visited` 检测循环（沿用现有） |
| B 诊断误报（如合理重名） | 低 | 仅对真正冲突/歧义报错，显式优先规则不变；`NameNotFound` 候选仅为提示不报错 |

---

## 8. 验收总览

- **A**：`pub_module_export.rl` 通过；全量 0 失败。
- **B**：`import_explicit_shadows_glob.rl` 通过；3 个 compile-fail（`alias_conflict`/`glob_ambiguity`/`not_found`）锁定。
- **C**：`private_item_access.rl` 锁定；std 在 `--visibility=error` 下 0 失败。
- **D**：本地 `--dep-root` 示例端到端编译运行通过；无依赖时行为不变。
- **统一**：每批次结束 `cargo test --workspace` 全绿（含 `rlyeh_test_suite_all_pass` 全量 .rl 套件）。

---

## 9. 待评审决策点（汇总）

1. **C1（`pub module` 语义）**：A 采用「增量放开」（仅 `pub module` 额外开放对外可达，不收紧非 `pub` 模块）还是「严格收紧」（非 `pub` 模块外部不可达，配 `private_module_import` 报错）？**推荐增量放开**。
2. **C2（可见性默认）**：`--visibility` 开关默认 `off`，还是新项目默认 `error`、存量项目 `off`？**推荐全局默认 `off`，待 std 迁移完成后统一翻转**。
3. **C3（glob 歧义严格度）**：两 glob 同名使用处直接报错，还是仅当该名被实际使用时才报？**推荐实际使用时报（惰性），避免未使用符号误报**。
4. **范围确认**：四块（A/B/C/D）是否本期全做，还是先 A+B（低风险）评审通过后再排 C+D？**按用户「前面 4 项都要」全做，但建议按 6 批次顺序推进**。

---

## 10. 变更记录

| 日期 | 变更 |
|------|------|
| 2026-09-19 | 根据用户确认（4 项都要）起草策划稿：A `pub module` / B 诊断 / C 可见性 / D dagon 包集成，含落点、语义决策、灰度策略、验收与待评审点 |
| 2026-09-20 | A 扩展：`pub` 标注覆盖 struct/enum/protocol/actor（trait→protocol，AST 仍为 `AstTraitDecl`）；parser 分发 + 收集登记 `pub_symbols` + struct 字面量/枚举变体/actor 构造函数可见性接线；CLI 验证通过，记录 spawn 与 protocol 约束两处架构限制 |
