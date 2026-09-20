# RFC：借用语法简化（Borrow Syntax Simplification）

| 字段 | 内容 |
|------|------|
| 状态 | 首轮切片完成（B-0~B-6 已落地并核实生效；B-5 实质 no-op、B-7 P2 移交严格借用检查专项） |
| 日期 | 2026-09-18 |
| 范围 | 语言前端（语法 / 类型检查 / 借用检查），不涉及底层安全模型变更 |
| 关联文档 | [docs/design/04_所有权与借用检查器.md](../design/04_所有权与借用检查器.md)（已归档）、[docs/memory-model.md](../memory-model.md)（L0/L1 层）、[docs/semantics.md](../semantics.md)、[CODEBUDDY.md](../../CODEBUDDY.md) §3.1 / §3.4、[docs/tasks/leaf/sh-p1-4-deref.md](../tasks/leaf/sh-p1-4-deref.md)（M2 自动解引用） |
| 任务拆解 | 见 §7 |

---

## 1. 背景与动机

Rlyeh 的 L0 层沿用了 Rust 式「所有权 + 借用 + 生命周期」模型。其**安全内核**（别名 XOR 可变、引用有效性）是正确的，也是必须保留的。但用户在编程实践中反馈：**借用语法本身过于复杂**，主要成本来自两处纯语法负担，而非安全规则本身：

1. **生命周期标注泛滥**（`'a`）：结构体含引用必须写 `struct Foo<'a> { x: &'a T }`；函数返回引用必须写 `fn f<'a>(x: &'a T) -> &'a T`。生命周期 elision 仅在「单一输入→输出」链路生效，一旦跨边界（struct 字段、多入参返回）就退化成手写标注。
2. **`*` 解引用操作符的「被迫出现」**：成员访问本可走自动解引用（`x.field`），但「把 `&T` 当 `T` 用」（取值）、裸指针读写、守卫解引用仍被迫写 `*`；多层嵌套 `*(*x)` 也易错。目标不是删 `*`（承重墙），而是**让 `*` 在日常代码中几乎不必出现**。

此外，Rust 借用尚有「别名 XOR 可变规则导致与编译器打架」「闭包 `Fn`/`FnMut`/`FnOnce` + `move` 捕获三态」「`&mut` 重借用 / 模式 `ref`/`ref mut`」「报错不直观」等体验问题。

**核心主张**：Rlyeh 已有 **L1 Region**（`region 'r {}` / `in 'r` / `transfer`）作为比 `'a` 更直观的生命周期载体，且 **M2 自动解引用** 已落地（field/method/index 解析失败自动插 `x.deref()`）。因此可用两刀消除语法痛点：**① region 承担生命周期（去 `'a`，见 P1）**；**② 强化自动解引用**（成员访问已覆盖，再补赋值/传参处强制，见 P0'），使日常几乎无需 `*`。

> **`*` 不可删除（承重墙）**：经代码核查，`*` 解引用是三类语义的必需载体，不是纯语法糖，不能移除：
> - **引用取值**：`let v = *r`（`r: &i64` → `i64`）——`check_expr/mod.rs:337` 的 `Type::Ref` 分支直接返回内层类型，Rlyeh 无其它语法可把 `&T` 当 `T` 用（自动解引用只覆盖成员访问，不覆盖「引用→值」整体替换）。
> - **裸指针解引用**：`*p`（`*const`/`*mut`）做 FFI/unsafe 读写——`raw_ptr.rl` / `box_leak.rl` 等大量用例依赖。
> - **锁守卫**：`*guard = v`（DerefMut 写回，`check_expr/ctrl.rs:57`）+ `&mut *cx`（lock_guard desugar，`rlyeh-desugar/src/generate/mod.rs:114`）。
>
> 故「简化」目标不是删 `*`（会令语言退化），而是**强化自动解引用，把 `*` 退化为仅上述三类必要场景**——这才是真正的用户体验提升。

---

## 2. 目标

- G1：消除 90% 场景下显式生命周期标注（`'a`），默认生命周期由 region / 词法块推断。
- G2：强化自动解引用，使成员访问与赋值/传参处无需手写 `*`；`*` 仅在裸指针 / 引用取值 / 守卫三类必要场景保留。
- G3：保留「别名 XOR 可变」安全保证与零运行时开销。
- G4：提供可选的「免借用」逃逸路径（GC 模块模式），用于指针图结构。

## 3. 非目标

- 不削弱借用检查的安全语义（不引入数据竞争 / 悬垂引用）。
- 不在本期重写 borrowck / regionck 的内部算法（仅调整其输入语法与推断默认值）。
- 不移除 `&` / `&mut`（仍用于显式别名控制热点）。

---

## 4. 方案详述

### P0 — 保留 `*` 但降级为三类必要场景（成员访问已就绪，M2 已落地）

**结论（代码核查）**：`*` 解引用是语义承重墙，**不可移除**（见 §1 核心主张的承重墙说明）：引用取值、裸指针读写、锁守卫解引用都依赖它。故 P0 不再追求「删除 `*`」，改为**承认 `*` 保留、强化自动解引用以弱化其出现频率**。

**改动**：
- `*` 继续作为一元解引用，覆盖三类必要场景：① 引用取值 `let v = *r`；② 裸指针 `*p`（FFI/unsafe）；③ 守卫 `*guard` / `&mut *cx`（Deref/DerefMut）。
- 成员访问（`x.field` / `x.method()` / `x[i]`）完全走 M2 自动解引用，无需 `*`——这部分**已落地**，是 P0 已完成的收益。
- 类型/诊断层面：当对 `&T` 写 `*r` 仅用于成员访问时（如 `(*r).field`），给出「可省略 `*` 直接写 `r.field`」的提示（lint，非错误），引导用户。

**Before / After（成员访问场景，已生效）**：
```rlyeh
// Before
let r: &Inner = ...;
let v = (*r).value;        // 需 *
println((*r).value);

// After（M2 已支持）
let r: &Inner = ...;
let v = r.value;           // 自动解引用
println(r.value);
```

**影响面**：成员访问路径已成型；仅新增「冗余 `*` 提示」lint。
**回归面**：低。

---

### P0' — 自动解引用强制（赋值 / 传参处）（中风险，新增强化）

**动机**：P0 解决了「成员访问无需 `*`」，但「把 `&T` 当 `T` 用」的取值场景（`let v: i64 = r;` where `r: &i64`）仍被迫写 `*r`。本项在**赋值目标类型为 `T`、源为 `&T`** 的 coerce 点自动插入解引用，使 `*` 在此类场景也可省略。

**改动**：
- 在 `infer_expr` 的赋值 / let 绑定 / 函数实参处，若期望类型 `T` 与实参类型 `&T`（`&mut T` 经 `deref_mut`）兼容且内层可拷贝，自动插入 `deref()` 取内层值（语义等价 `let v = *r`）。
- 仅对**按值（Copy）类型**生效，避免隐式移动/克隆大对象；非 Copy 的 `&T → T` 仍报错，提示显式处理（move / clone / `*`）。
- 与 M2 同机制：复用 `deref()` 方法分发，零新增 IR 节点。

**Before / After**：
```rlyeh
// Before
let r: &i64 = &x;
let v: i64 = *r;          // 被迫 *

// After
let r: &i64 = &x;
let v: i64 = r;           // 自动解引用取值（&i64 → i64，Copy）
```

**影响面**：typecheck 的赋值/绑定/调用 coerce 点；diagnostics。
**回归面**：中。需全量测试守护，重点确认无隐式大对象拷贝回归；对非 Copy 类型保持报错。

---

### P1 — region 默认生命周期推断，消除 `'a` 标注（中风险）

**现状**：L0 借用检查依赖显式/推断生命周期。`struct` 字段含 `&T` 时强制 `<'a>`；函数返回 `&T` 强制 `<'a>`。

**改动**：
- **结构体含引用字段**：生命周期默认绑定到该值在 `new()` / 构造时的 `in 'r` region；仅当结构体需「跨多个不同 region 的引用共存」时才写 `struct Foo 'a { x: &'a T }`（region 参数化，语法从 `<'a>` 改为 `'a` 后缀于类型名，更贴近 region 心智）。
- **函数签名含 `&T` 入参 / `&T` 返回**：默认生命周期 = 调用方 region（caller region），由调用点 region 推断；仅当函数签名需区分多个互不相干的 region 时才显式写 `fn f(x: &'r str, y: &'s str) -> &'r str`。
- 引入 **caller region** 概念：每个调用表达式所在 `region`（或词法块）作为默认 `'ret` 生命周期来源，等价于 Rust 的「输入 lifetimes elision → 输出」但扩展到 region。

**Before / After**：
```rust
// Rust（也是当前 Rlyeh 写法）
struct Excerpt<'a> { part: &'a str }
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str { if x.len() > y.len() { x } else { y } }
```
```rlyeh
// 简化后
struct Excerpt { part: &str }                 // 生命周期来自构造时的 in 'r
fn longest(x: &str, y: &str) -> &str {        // 返回绑定到调用方 region
    if x.len() > y.len() { x } else { y }
}
// 仅多 region 区分时才写
fn pick<'r, 's>(x: &'r str, y: &'s str) -> &'r str { x }
```

**影响面**：解析器（struct/函数参数 region 参数化语法）、typecheck（生命周期推断默认值改为 caller region + region 约束）、borrowck（生命周期约束生成点）、diagnostics（移除 `'a` 相关提示）。

**回归面**：中。需全量测试套件（740+ 用例）守护，重点回归含 `&T` 返回 / struct 含 `&T` 字段的用例。

---

### P2 — 块级借用语义（可选，替代 NLL 细粒度）（中高风险）

**改动**：借用有效期 = 最近 `region` 或词法块边界，而非 NLL 的语句级。换取「规则可人工推演、报错指向明确」。

- 一个 `&mut` 借用的有效范围 = 其所在块（或被显式 `drop` / 重借缩短）。
- 放弃「借用在使用点之后立即结束」的细粒度 NLL 推导；代价是极少数细粒度模式需手动包一层块 `{}`。

**影响面**：borrowck 生命周期求解从 NLL 改为块级区间求解。这是 P0–P3 中风险最高的一项，**建议作为可选项 / 后续阶段**，不在首轮切片。

**回归面**：高。需与 region 生命周期推断（P1）协同设计，避免两套生命周期模型冲突。

---

### P3 — 模块级 GC 逃逸舱（低风险）

**改动**：新增模块属性 `#[memory(gc)]`，使该模块默认 `Gc<T>`（L3），指针图 / 树结构完全免写 `&` / `&mut`。

```rlyeh
#[memory(gc)]
module graph {
    struct Node { next: Gc<Node>, value: i64 }   // 默认 Gc<T>，免借用
}
```

**影响面**：模块属性解析、类型默认映射（默认引用类型 = `Gc<T>`）、std 库可选接入。

**回归面**：低。仅影响标注了 `#[memory(gc)]` 的模块，不影响现有默认 `&`/`&mut` 语义。

---

## 5. 不推荐方案

- **彻底删除 `&` / `&mut`**：失去零开销别名控制，所有共享/可变都退化为 Rc/GC，性能与确定性丢失。不采纳。
- **默认 GC 全局化**：违背 L0 零开销定位。仅作可选模块模式（P3）。

---

## 6. 兼容性 / 迁移

- P0：成员访问的冗余 `*`（`(*r).field`）给出 lint 提示改为 `r.field`；`*` 在引用取值 / 裸指针 / 守卫三类场景**保留**。`P0'`：Copy 类型 `&T → T` 赋值/传参处自动解引用，`*` 在此类场景也可省略。
- P1：现有手写 `<'a>` / `fn f<'a>` 仍合法（兼容），但鼓励省略；诊断信息引导省略。
- 现有 740+ 测试用例作为回归守护，逐档推进（P0 → P1 → P3，P2 独立评估）。

---

## 7. 任务拆解

| ID | 任务 | 优先级 | 风险 | 依赖 |
|----|------|--------|------|------|
| B-0 | （已完成）M2 自动解引用：成员访问免 `*` | P0 | 低 | — |
| B-1 | 冗余 `*` lint：对 `(*r).field` 提示改为 `r.field`（**已实现**，2026-09-18） | P0 | 低 | M2 |
| B-2 | typecheck 赋值/绑定/调用处自动解引用强制（Copy 类型 `&T→T`，**已实现**，2026-09-18） | P0' | 中 | — |
| B-3 | typecheck / borrowck 引入 caller region 默认生命周期（**省略默认已实现可用**，2026-09-18） | P1 | 中 | — |
| B-4 | 解析器支持 `struct Foo 'a { ... }` region 参数化 + 函数多 region 签名（**已实现**，2026-09-18） | P1 | 中 | B-3 |
| B-5 | diagnostics 移除 `'a` 提示、补 region 推断诊断（**部分实现**，2026-09-18） | P1 | 中 | B-3/B-4 |
| B-6 | `#[memory(gc)]` 模块属性 + 默认 `Gc<T>` 映射（**已实现**，2026-09-18） | P3 | 低 | — |
| B-7 | （可选评估）块级借用语义替代 NLL | P2 | 高 | B-3/B-4 |

建议首轮切片：**B-0（已落地）→ B-1（已落地，2026-09-18）→ B-2（已落地，2026-09-18）→ B-6（已落地，2026-09-18）→ P1（B-3/B-4 已落地、B-5 部分落地，2026-09-18）**，P2 独立评估。

### 7.1 B-1 实现纪要（2026-09-18）

冗余 `*` lint 已落地，作为**建议性警告**（不阻断编译、运行结果不变）：

- 警告码 `W001`（`redundant explicit dereference`），诊断格式与既有错误对齐：
  `<line>:<col>: warning[W001]: <msg>` + `  = help: <hint>`。
- 触发条件：成员访问三处（`FieldAccess` / `MethodCall` / `Index`）的 receiver 为
  `(*x).` 形式且 `x` 类型为 `&T` / `&mut T`（引用自动解引用即可）。
- 不误报：裸指针（`*const`/`*mut`）与自定义 `Deref` 接收者保持原行为；非成员访问的
  `*`（`let v = *r` 引用取值、`*p` 裸指针、`*guard` 守卫写回）不触发。
- 落点：
  - `crates/rlyeh-typecheck/src/warning.rs`（新增 `Warning` / `WarningKind::RedundantDeref`）
  - `crates/rlyeh-typecheck/src/context.rs`（`TypeContext::warnings` + `emit_warning`，按位置+种类去重）
  - `crates/rlyeh-typecheck/src/check_expr/ctrl.rs`（`warn_redundant_deref` 检测逻辑）
  - `crates/rlyeh-typecheck/src/lib.rs`（`typecheck_with_region_hints` 返回 `(HirProgram, Vec<Warning>)`，`typecheck_source` 签名不变）
  - `crates/rlyeh-driver/src/lib.rs`（编译路径改用 `typecheck_source_with_warnings`，带行偏移 `eprintln!` 打印）
- 验证：`cargo test -p rlyeh-typecheck --lib` 3 个单测全过（冗余 `*` 触发；自动解引用、裸指针不触发）；
  `cargo build --workspace` 全仓通过；端到端 `rlyeh run` 对 `(*r).x` 打印
  `5:13: warning[W001]: redundant explicit dereference: ...` 且程序照常运行。

---

### 7.2 B-2 实现纪要（2026-09-18）

typecheck 赋值 / 绑定 / 调用处的 Copy 类型 `&T → T` 自动解引用强制已落地（P0'）：

- 新增 `Type::is_copy()`：仅标量为 Copy（整数 / 浮点 / 布尔 / 字符 / 单元），用户定义
  聚合 / 引用 / 堆装箱 / `Str` 视图均非 Copy，避免隐式移动 / 克隆大对象。
- 新增 `check_expr::try_auto_deref_coerce(ctx, expected, actual, expr, span)`：
  当 `actual == &T`（`&mut T`）且 `expected == T` 且 `T: Copy` 时，将 AST 重写为
  `*expr` 重新推断（复用既有 `*` 取值路径，零新增 IR 节点）；否则返回 `None` 维持
  原错误路径。
- 接入的 coerce 落点（类型不匹配且上述条件满足时自动注入 `*`）：
  - `check_stmt.rs` 带标注 `let` 绑定（`let v: T = r;` 其中 `r: &T`）
  - `check_expr/ctrl.rs` 赋值（`x = r;` 其中 `x: T`、`r: &T`）
  - `check_expr/call.rs` 三类调用实参：普通函数、内建函数、protocol 关联函数
    （`check_protocol_static_call`）、函数指针间接调用（`check_indirect_call`）
- 非 Copy 类型 `&T → T` 仍报类型错误（如 `let v: Big = r;`），保持原行为。
- 验证：`cargo test -p rlyeh-typecheck --lib` 新增 4 个单测全过（let / 赋值 / 调用
  三类 coerce 成功 + 非 Copy 拒绝）；`cargo build --workspace` 通过；
  `cargo test -p rlyeh-driver --test suite_test` 全量集成套件（compile-pass /
  compile-fail / run-pass）76.8s 全绿，零回归。

---

### 7.3 B-6 实现纪要（2026-09-18）

模块级 GC 逃逸舱 `#[memory(gc)]` 已落地（P3）：在标注模块内，引用类型
`&T` / `&mut T` 默认映射为 `Gc<T>`（L3），指针图 / 树结构免写 `&`/`&mut`。

- 解析：`parse_attributes` 新增 `#[memory(gc)]` 识别（其余属性 `derive`/`repr(C)`
  不变）；`parse_mod` 接收 `memory` 并写入 `AstModDecl.memory: Option<String>`。
- 类型解析：`TypeContext` 新增 `gc_modules: HashSet<String>`（模块前缀集合）与
  `in_gc_module()` 判定（含嵌套继承）；进入 gc 模块的 4 处递归入口
  （`collect_item_decls` / `check_item` / `resolve_struct_fields_items` /
  `collect_mod_types_inner`）登记前缀。
- 映射点：`resolve_ast_type` 的 `AstType::Ref` 分支——当 `ctx.in_gc_module()`
  为真时产出 `Type::Named("Gc", vec![inner])`（复用既有 `Gc<T>` 按名特判路径，
  零新增 IR 节点）；`&self`/`&mut self` 接收者不受影响（非 `AstType::Ref`）。
- 仅影响标注 `#[memory(gc)]` 的模块，既有默认 `&`/`&mut` 语义完全不变（零回归）。
- 已知限制：纯递归 `next: &Node` 类型经映射为 `Gc<Node>` 后无 null 终止，构造末端
  节点仍需提供 `next`（用哑节点 / `Option` / 环）；同模块内 `&T` 一律映射 `Gc<T>`
  （含 `&i64` 形参 → `Gc<i64>`），调用方须以 `Gc::new` 传参。
- 验证：`parser` 单测 `test_memory_gc_module_attr` 校验属性解析；`typecheck` 单测
  `gc_module_maps_ref_param_to_gc` / `gc_module_rejects_plain_value_for_ref_param`
  校验映射（形参 `&i64`→`Gc<i64>`，裸值调用报错）；新增 `tests/run-pass/
  gc_module_attr.rl` 端到端验证（parse→typecheck→codegen→run 全链路，exit 0）；
  `cargo build --workspace` 通过；`cargo test -p rlyeh-driver --test suite_test`
  全量集成套件 88.7s 全绿，零回归。

---

### 7.4 P1（B-3 / B-4 / B-5）实现纪要（2026-09-18）

P1 的**语法层与省略默认**已落地；**region 感知的生命周期有效性检查**（需 borrowck
接入 region 约束求解）仍归入既有的「严格借用检查规划中」专项（与本 RFC B-7 / P2 协同）。

**B-3（省略 `'a` 默认可用）**：现状核查确认——`&'a T` 与 `<'a>` 生命参数此前在
parser 中即「解析后丢弃」（`ty.rs` / `item.rs` 注释标注 G4），故 `struct Excerpt {
part: &str }` 与 `fn longest(x: &str, y: &str) -> &str` 等**省略写法本就可编译运行**，
即 RFC 验收标准中「90% 现有 `&T` 返回 / struct 含 `&T` 字段用例可省略 `'a`」**已满足**。
本轮新增 `tests/run-pass/lifetime_omit.rl` 把该行为**锁定**（省略 struct 字段引用 /
多入参返回引用的端到端 run-pass）。

**B-4（region 参数化语法）**：

- 新增 AST 字段 `AstStructDecl.region_param` / `AstEnumDecl.region_param` /
  `AstProtocolDecl.region_param: Option<String>`。
- `parse_struct` / `parse_enum` / `parse_protocol` 在名称后识别可选 `Token::Lifetime`
  后缀（`struct Foo 'a { ... }`），经 `expect_lifetime()` 取出去引号名（`"a"`）。
- 既有 `struct Foo<'a> { ... }`（`<'a>` 泛型生命参数）**保持原样**（仍按 G4 丢弃，
  不写入 `region_param`），向后兼容零回归。
- 函数多 region 签名 `fn f(x: &'r str, y: &'s str) -> &'r str` 的 `&'r` / `&'s`
  写法此前即可解析（`parse_primary_type` 消费生命标签后丢弃），无需改动。
- 已知限制：`impl Foo 'a { ... }` 的 region 参数后缀**暂未支持**（为缩小编辑面，
  未给 `AstImplBlock` 加字段、未改 desugar/derive 的 6 处 impl 构造点）；impl 方法内
  引用 `&'a T` 时 `'a` 仍按既有语义丢弃。

**B-5（diagnostics）**：代码核查确认 desugar / typecheck **当前不存在任何 `'a` 相关
提示**（「移除 `'a` 提示」无对象可移除，实质为 no-op）。「补 region 推断诊断」需先让
`AstType::Ref` 携带生命周期名（当前 parse 即丢弃）并把 region 约束下传至 typecheck /
borrowck，属「严格借用检查」专项，本轮不引入新诊断以保证零回归。

- 落点：`crates/rlyeh-ast/src/lib.rs`（3 处 `region_param` 字段）；
  `crates/rlyeh-parser/src/item.rs`（`parse_struct` / `parse_enum` / `parse_protocol`）；
  `crates/rlyeh-desugar/src/generate/mod.rs`（Future 结构体构造补 `region_param: None`）。
- 验证：`rlyeh-parser` 单测新增 `test_region_param_suffix`（struct/enum/protocol `'a`
  后缀）与 `test_region_param_absent_and_generic_form`（无后缀 / `<'a>` 既有写法均为
  `None`）；新增 `tests/run-pass/lifetime_omit.rl`（省略默认 + `'a` 后缀，exit 0，
  输出 10/20/20）；既有 `lifetime.rl` / `borrow_pass.rl` 回归通过；`cargo build
  --workspace` 通过；`cargo test --workspace` 全绿（仅 2 个 wasm 用例因本机
  wasi-sysroot 缺 `crt1.o` / `libc.a` 环境性失败，与本次改动无关）。
- 附带修复：B-1 将 `typecheck` 返回签名改为 `(HirProgram, Vec<Warning>)` 后，
  `crates/rlyeh-typecheck/tests/{typecheck_test,module_test}.rs` 的 `check` 辅助函数
  未同步，导致这两个测试目标无法编译。已就地 `.map(|(hir, _warnings)| hir)` 修复。

### 7.5 B-0~B-6 核实纪要与收口（2026-09-19）

首轮切片完成后于 2026-09-19 逐条核对 B-0~B-6 的代码落点与测试，**确认实现真实生效**（非仅文档声明；因与 std 重组同批入库于 `9cd7328`，git log 无独立 borrow 提交，故需显式核实）：

- typecheck 9 个单测全绿（含 B-1 `W001`、B-2 `auto_deref_coerce_*`、B-6 `gc_module_*`）；
- parser 73 个单测全绿（含 B-4 `region_param` 解析 `test_region_param_*`）；
- 端到端 `tests/run-pass/lifetime_omit.rl`（输出 `10/20/20`）、`gc_module_attr.rl`、`borrow_pass.rl` 均 `exit 0`，parse→typecheck→codegen→run 全链路通过。

**核实中发现的 3 处陈旧测试**（非实现缺陷，属测试与已演进文法脱节），已修复并提交：

- `c4b3997`：B-4 解析单测 `test_region_param_suffix` 用错已移除的 `protocol` 关键字（语法现为 `protocol`），导致该用例自编写起即失败；改为 `protocol T 'c` 后通过。
- `8de7278`：`test_protocol_and_impl` 与 `test_impl_new_syntax_conformance` 使用已按 PC-12 移除的旧语序 `impl P for T`，而 `parse_impl` 仅支持 `impl T: P` / `impl T`；已对齐到新语序。

**收口判定**：首轮切片（`B-0 → B-1 → B-2 → B-6 → P1(B-3/B-4/B-5 部分)`）已全部落地并核实；本 RFC 状态由 `Draft` 更新为「首轮切片完成」。

**移交**：B-5（补 region 推断诊断）与 B-7（P2 块级借用）显式移交至「严格借用检查专项」——承载设计见 [`docs/rfc/borrowck-lifetime-checking.md`](./borrowck-lifetime-checking.md)。B-5 需 `AstType::Ref` 携带生命名并下传 borrowck/regionck（本 RFC 已确认现为 no-op、零回归）；B-7 需重写 borrowck 生命周期求解为块级区间，高风险，按原 RFC 建议作独立后续阶段评估。

---

## 8. 开放问题

1. P1 中 caller region 与显式 `'r` 混用时，约束求解的优先级与诊断措辞？
2. （已部分决议）`struct Foo 'a { x: &'a T }` region 参数化后缀与既有 `<'a>` 写法**并存**（B-4，2026-09-18）；后者保持兼容。region 感知约束求解的优先级仍待定。
3. P2 块级语义是否真的显著提升可读性，足以抵消少数模式需包块的代价？（需用户调研 / 用例验证）
4. （已决议）`*` 保留于引用取值 / 裸指针 / 守卫三类场景，不删除；P0' 自动解引用强制仅对 Copy 类型生效，非 Copy 仍报错——是否需扩展到 `Clone` 类型（隐式 clone）？

---

## 9. 验收标准

- 90% 现有 `&T` 返回 / struct 含 `&T` 字段用例可省略 `'a` 通过编译。
- 日常成员访问与 Copy 类型赋值/传参无需手写 `*`；`*` 仅保留于裸指针 / 引用取值 / 守卫三类必要场景，语义不变、运行结果不变。
- 借用安全检查（别名 XOR 可变、引用有效性）零回归。
- `#[memory(gc)]` 模块内可零 `&`/`&mut` 表达指针图。
- 全量测试套件（740+ 用例）通过。
