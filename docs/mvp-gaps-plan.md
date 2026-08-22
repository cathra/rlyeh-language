# 开发计划 — MVP 已知限制消解（阶段 G–L）

> **性质**：本文档为 [`guide.md`](./guide.md) §13「已知限制（MVP）」的消解计划，承接 [`development-plan.md`](./development-plan.md)（阶段 A–F 全部完成后）的续作，为**第三版规划的权威副本**。
> **需求来源**：`docs/guide.md` §13 已知限制（MVP）列出的 7 项规划特性 + 5 项实现约束。
> **总纲与进度速览**：见根目录 [`CODEBUDDY.md`](../CODEBUDDY.md)（§3.9 限制速览表、§5.5 执行记录）。
> **维护规则**：每完成一项任务，需同步更新本文档状态标识 + `CODEBUDDY.md` §3.9 / §5.5 + `guide.md` §13（勾销对应限制条目）。

---

## 1. 背景：MVP 已知限制盘点

### 1.1 规划中 / 未实现特性（7 项）

| # | 限制（guide.md §13） | 编译器现状依据 | 归属阶段 |
|---|---------------------|---------------|---------|
| 1 | **宏系统**：`println!` / `vec!` / `format!` 等宏调用不支持（`!` 是 `not` 一元运算符）；打印用内建 `println(expr)`（0–1 参数，无 `{}` 格式化） | grammar.md §2.14 已有 `macro_rules!` EBNF（规划标注）；typecheck `check_call` 已有 `name.ends_with('!')` 分支；std-lib.md §8 `Display`/`Debug`/`format!` 为规划 API | **I** |
| 2 | **引用与借用**：`&x` 表达式、`&T` 参数类型、`str` 类型、解引用 `*`、裸指针均未实现；仅方法接收者 `&self`/`&mut self` 可用 | grammar.md Type 规则含 `'&' Lifetime? 'mut'? Type`、UnaryExpr 含 `'*' | '&' 'mut'?`、Pattern 含 `'ref'`（均已定义未实现）；typecheck `UnaryOp::Deref/AddrOf/AddrOfMut` → Unsupported（check_expr.rs）；borrowck crate 仅服务 `&self` 接收者 | **G** |
| 3 | **闭包**：`|x| x + 1` 语法可解析，typecheck 报 Unsupported | parser 已产出 `AstExpr::Closure`；typecheck 报 Unsupported（check_expr.rs） | **H** |
| 4 | **运算符**：✅ 已解决——K1 `?` 错误传播、H1 函数指针、H4 `dyn Trait` 均已实现（见 §4 执行记录） | grammar.md 含 `'dyn' TraitBound` 与后缀 `'?'`（已实现）；`Option`/`Result` + `expect`/`unwrap_or` 已实现 | ✅ |
| 5 | **所有权层级**：✅ 已解决——K2 `Box<T>` / K3 `Rc<T>`/`Arc<T>`/`Weak<T>`（引用计数）/ K4 `Gc<T>`（MVP 保守标记-清除 + `gc_region` 生命周期）均已实现（见 §4 执行记录） | memory-model.md §4（Rc/Arc）/§5（Gc）规范完备（含布局、转换矩阵、开销表）；`Box<T>` 亦为 §3 目标 API | **K** |
| 6 | **并发**：`serde`/`fmt`/`async` 模块为规划；actor 的 `async` 方法 + `.await`/`send` 已实现 | std-lib.md §8（fmt）/§9（serde）/§10（async 运行时）规划标注；actor 异步为独立机制 | fmt→**I**，serde/async→**L** |
| 7 | **迭代器协议**：数值区间、`for x in vec`/`for (k, v) in map`/`for x in arr`（数组迭代 J1）可用；自定义迭代器（`next() -> Option<T>` 方法）接入 `for`（J2）；`map`/`filter`/`fold`/`collect`/`take`/`skip` 适配器可用（J3，返回 Vec） | typecheck for 循环分派（range/Vec/HashMap/数组/迭代器），适配器内建 desugar（check_expr.rs）；std-lib.md §2.3 Iterator 规划 | **J** |

### 1.2 实现约束（5 项）

| # | 约束（guide.md §13） | 现状 | 处置 |
|---|---------------------|------|------|
| 1 | std 模块化（core.zeta + time/io/net/sync 子模块） | ✅ 已完成 | 说明性，不纳入计划 |
| 2 | net：`tcp_connect` 依赖平台 `sockaddr_in4` 布局（已双布局化）；WASI 下网络不可用 | 平台约束 | **L**（平台加固） |
| 3 | Actor 运行时：交叉编译 / WASM 目标下 actor 程序暂不支持 | 平台约束 | **L**（平台加固） |
| 4 | `String::from(s)`：非字面量 Str（运行期内容）长度表达未实现 | typecheck 报 Unsupported | **G**（str 切片一并补齐） |
| 5 | region 选项：`adaptive`/`with_size (N)` 可用；`strategy (bump)` 等其余选项规划中 | `zeta-region-alloc` 已就绪（PGO 回灌 F2 已打通） | **L** |

---

## 2. 计划总览（阶段 G–L）

| 阶段 | 主题 | 关键交付 | 依赖 | 状态 |
|------|------|---------|------|------|
| **G** | 引用与借用（L0 完整化） | `&T`/`&mut T`、`*` 解引用、`str` 切片、裸指针、生命周期 `'a` | 无（地基） | G1–G4 ✅（G4 为语法接受 MVP，borrowck 生命周期检查规划中） |
| **H** | 一等函数 | `fn(A) -> B` 函数类型、闭包（捕获 + `move`）、`dyn Trait` | G（引用捕获） | H1–H4 ✅（H3 为 IIFE MVP；H4 为 dyn Trait MVP） |
| **I** | 宏系统与格式化 | `macro_rules!` 声明式宏、`Display`/`Debug`、`println!`/`format!` | 弱（可与 G/H 并行） | ✅ 已完成 |
| **J** | 迭代器与集合协议 | 数组迭代、`Iterator` trait、`map`/`filter`/`fold`/`collect` | H（适配器闭包） | J1–J3 ✅ |
| **K** | 错误传播与所有权层级 | `?` 运算符、`Box<T>`、`Rc<T>`/`Arc<T>`、`Gc<T>` | G（指针操作） | K1–K4 ✅ |
| **L** | 生态模块与平台收尾 | `async fn`/`await`、`serde`、region `strategy (bump)`、WASI net / Actor 交叉编译 | I（serde 宏）、G 等 | ⏳ 未开始 |

> **推荐执行路线**：
> - **保守路线（依赖驱动）**：G → H → J → K → L，I 按需插入（宏展开器在 parse 后、typecheck 前，与 G/H 解耦，可随时并行）。
> - **快赢路线（体验驱动）**：先做 I1/I2（宏 + `println!` 格式化，开发者日常收益最大、不依赖引用系统）与 G 同步推进；J1（数组迭代）独立且极小，可先行交付。
> - 优先级建议：G1–G2（引用地基 + str）> I1–I2（宏 + 格式化）> H1–H2（函数指针 + 无捕获闭包）> K1（`?` 运算符）> J 全阶段 > 其余。

---

## 3. 阶段详情

### 阶段 G — 引用与借用（L0 完整化）

> 现状：`&x`/`&mut x`/`*` 已实现（G1 ✅）：`&T`/`&mut T` 类型、取址/解引用表达式、参数与返回值引用、标量与聚合引用内存模型、字段访问/方法调用的自动 `peel_ref`、借用宽松规则（`&mut T` 兼容 `&T` 参数，严格可变性互斥留给 borrowck）均可用；`&self`/`&mut self` 方法接收者保持可用；`&str` 视图已实现（G2 ✅）：`String::as_str()` 只读借用（瘦指针，运行时 = 指向 String 对象的指针）、`&str` 参数/返回、字节索引、`String::from(&str)` 深拷贝、视图与 String 内容比较互用；`String::from(String)` 运行期长度表达可用。**裸指针已实现（G3 ✅ MVP）**：`*const T`/`*mut T` 类型 + `*p` 读写（标量 load/store，codegen 与引用同为 `i8*` 槽）+ `&T` ↔ `*const T` 互视（FFI 宽松规则）+ `*mut T` 降级 `*const T`；**生命周期标注已实现（G4 ✅ MVP 语法接受）**：`<'a>` 泛型参数与 `&'a T`/`&'a mut T` 类型解析后丢弃（宽松检查，`'a` 绑定验证规划中）。borrowck crate 存在但仅服务接收者。`ref` 模式、严格借用检查与 `strategy (bump)` 等仍规划中。

| 任务 | 内容 | 状态 |
|------|------|------|
| G1 | **引用类型与表达式**：`&T`/`&mut T` 类型、`&x`/`&mut x` 表达式、`*` 解引用；函数参数 `x: &T`、返回 `&T`；`ref` 模式。typecheck 类型规则 + borrowck 借用规则（可变性、悬垂、别名）接线 | ✅ |
| G2 | **`str` 切片与 String 补齐**：`&str` 引用切片（新增 `as_str()` 只读借用视图，`substring` 保持拷贝返回以免破坏现有 API）；`String::from(s)` 支持运行期 String/str 内容（长度表达），消除 §13 约束 4 | ✅ |
| G3 | **裸指针**：`*const T`/`*mut T` 类型 + `*p` 读写（FFI 场景；codegen 直通） | ✅（MVP：`*const T`/`*mut T` 类型注解、`*p` 读写、`&T` ↔ `*const T` 互视、`*mut` 降级 `*const`；codegen 与引用同构） |
| G4 | **生命周期标注**：`'a` 参数（grammar.md 已定义）、省略规则；borrowck 生命周期检查 | ✅（MVP 语法接受：`<'a>` 与 `&'a T` 解析后丢弃；borrowck 生命周期检查规划中） |

**风险与对策**：借用检查是全语言最复杂的子系统，建议分四步走——先允许（宽松借用，typecheck 报错兜底）→ 严格借用检查 → 生命周期 → 指针；每步以 `cargo test --workspace` + clippy 0 警告为准入。

### 阶段 H — 一等函数（闭包 + 函数指针）

> 现状：`fn(A) -> B` 函数类型 + 函数一等值已实现（H1 ✅：`let f = add` 函数值绑定、`f(args)` 间接调用、作实参/返回值/重新绑定/类型注解；codegen 函数指针 `i8*` 槽 + 按签名 `bitcast` + 间接 `call`）；无捕获闭包已实现（H2 ✅：`|x, y| expr` desugar 为匿名函数 `__closure_N` + 函数指针，需 fn 类型上下文驱动参数推断，捕获外部变量报错）；**捕获闭包已实现（H3 ✅ IIFE MVP）**：`(|x| body)(args)` 立即调用时 body 中引用的外层变量按值捕获（迭代收集 + 类型快照），desugar 为匿名函数 `__closure_N(cap1, cap2, x)`（捕获变量作前置参数、参数名保留原名）+ 普通函数调用，零新增 IR 节点；字符串字面量实参自动升级为 String 语义（与 `let s = "..."` 绑定一致）；`move` 关键字 MVP 忽略（所有权宽松）；`let f = |x| ..; f(..)` 闭包值对象（匿名结构体 + call 方法）与按引用捕获规划中；**`dyn Trait` 已实现（H4 ✅ trait 对象 MVP）**：`dyn Trait` 类型 + `&T` 强制转换（vtable 运行时构造 + 2 槽胖指针）+ 方法调用经 vtable 间接分派；trait/impl 泛型单态化已实现，可为函数类型复用。

| 任务 | 内容 | 状态 |
|------|------|------|
| H1 | **函数类型与函数指针**：`fn(A) -> B` 类型（含泛型实例化）、函数作为值传递与调用（codegen 函数指针） | ✅ |
| H2 | **无捕获闭包**：`|x| expr` desugar 为普通函数 + 函数指针（零开销、无依赖 G） | ✅ |
| H3 | **捕获闭包**：按值捕获 desugar 为匿名结构体（捕获字段 + `call` 方法）；`move` 语义；按引用捕获依赖 G | ✅（IIFE MVP：立即调用 + 按值捕获；闭包值对象规划中） |
| H4 | **`dyn Trait`**：trait 对象（数据指针 + vtable：drop/size/align/方法表），`dyn Trait` 类型 + 强制转换 | ✅（MVP：非泛型 trait/impl；vtable 槽 0–2 置 0；方法按 trait 声明序入表） |

### 阶段 I — 宏系统与格式化

> 现状：grammar.md §2.14 已有 `macro_rules!` EBNF（规划标注）；typecheck `check_call` 对 `name.ends_with('!')` 已有宏调用分支（但未实现展开）；std-lib.md §8 的 `Display`/`Debug`/`println!`/`format!` 为规划 API。**与 G/H 解耦，可并行。**

| 任务 | 内容 | 状态 |
|------|------|------|
| I1 | **宏调用语法区分**：词法/语法层区分 `name!`（宏调用）与 `!`（not 一元运算符）；`macro_rules!` 声明式宏（matcher → transcriber，token 流展开于 parse 后、typecheck 前） | ⏳ |
| I2 | **`Display`/`Debug` trait + 格式化宏**：`{}` 占位符格式化引擎；`println!`/`print!`/`format!`/`dbg!` 宏；String 拼接语义（`{}` 插入 `to_string`） | ⏳ |
| I3 | **集合宏**：`vec!`/`map!`/`arr!`（展开为 `Vec::new` + push / `HashMap::insert` 序列） | ⏳ |

### 阶段 J — 迭代器与集合协议

> 现状：J1–J3 ✅ 已完成——for 循环分派（数值区间 / Vec / HashMap / 数组 / 自定义迭代器）已齐；适配器 `map`/`filter`/`fold`/`collect`/`take`/`skip` 经内建 desugar 可用（返回 `Vec<T>`，可链式：适配器结果 Vec 亦可作下一适配器源）。std-lib.md §2.3 `Iterator` trait 仍为规划 API（适配器为编译器内建，非 trait 实现）。

| 任务 | 内容 | 状态 |
|------|------|------|
| J1 | **数组迭代**：`for x in arr`（desugar 为数组下标循环，数组切片模式可复用） | ✅ |
| J2 | **`Iterator` trait**：`next() -> Option<Item>` 核心方法；自定义迭代器接入 `for`（经 trait 方法调用） | ✅ |
| J3 | **适配器**：`map`/`filter`/`fold`/`collect`/`take`/`skip`（依赖 H 闭包） | ✅ |

### 阶段 K — 错误传播与所有权层级

> 现状：`Option`/`Result` + `expect`/`unwrap_or` 已实现；`?` 运算符已实现（K1 ✅：Option/Result 早返回 desugar 为 `match` + `return`，零新增 HIR 节点）；裸无参变体值表达式（`return None;`）已支持（infer_expr Ident 分支补 split_variant_path 兜底）；`Box<T>` 已实现（K2 ✅：`Box::new` 堆分配 + `*` 解引用 + 字段/方法/索引自动剥层）；`Rc<T>`/`Arc<T>` 已实现（K3 ✅：引用计数内建 + 弱引用 + try_unwrap + 自动剥层，布局见 memory-model.md §4 MVP 注记）；`Gc<T>` 已实现（K4 ✅：MVP 保守标记-清除 + `gc_region` 生命周期 + 逃逸对象 + 嵌套块存活提升，布局与生命周期协议见 memory-model.md §5 MVP 注记），见 §4 执行记录。

| 任务 | 内容 | 状态 |
|------|------|------|
| K1 | **`?` 运算符**：Result/Option 早返回 desugar（`match` + `return`），要求函数返回兼容类型 | ✅ |
| K2 | **`Box<T>`**：堆分配目标 API（`Box::new`；解引用依赖 G） | ✅ |
| K3 | **`Rc<T>`/`Arc<T>`**：原子/非原子引用计数（memory-model.md §4 布局；`Rc::new`/`clone`/`try_unwrap`；`Deref` 特判或依赖 G） | ✅ |
| K4 | **`Gc<T>`（插件式）**：标记-清除 GC + `gc_region`（memory-model.md §5；write barrier、逃逸限制） | ✅（MVP 保守标记-清除；write barrier / 增量 / 多线程规划） |

### 阶段 L — 生态模块与平台收尾

| 任务 | 内容 | 状态 |
|------|------|------|
| L1 | **`async fn`/`await`**：普通函数异步支持（actor `async` 方法已有独立机制，抽象复用） | ⏳ |
| L2 | **`serde` 序列化模块**：`Serialize`/`Deserialize` trait + `#[derive]` 风格宏（依赖 I） | ⏳ |
| L3 | **region 选项接线**：`strategy (bump)` 等其余选项（`zeta-region-alloc` 已就绪，PGO 预测可直接回灌 `adaptive` 初始容量） | ⏳ |
| L4 | **平台加固**：WASI 下 net 支持（或明确禁用文档化）；Actor 交叉编译 / WASM 支持（消除 §13 约束 2/3） | ⏳ |

---

## 4. 执行记录

- [x] **H4 `dyn Trait` trait 对象（MVP）**（2026-08）：`dyn Trait` 类型 + `&T` 强制转换 + 方法调用 vtable 间接分派全链路（parser → typecheck → HIR → MIR/LIR → LLVM）。**类型与布局**：`AstType::Dyn(Box<AstType>)`（parser `ty.rs` `dyn` 关键字分支）+ typecheck `Type::Dyn(String)`（完整 trait 名；use 别名解析）；布局 = **2 槽胖指针**（槽 0 = 数据指针、槽 1 = vtable 指针，`field_scalar_of` Ptr）。**转换 `coerce_to_dyn`**：`let d: dyn Shape = &c;` / 函数实参 `&T` → `dyn Trait`（`compatible_with` 允许）；desugar 为 HIR 块——`Alloc(3+N)` vtable（槽 0–2 = drop/size/align，MVP 置 0）+ N 方法槽（`FnPtr("Trait::method")` 按 trait 声明序）+ `Alloc(2)` 胖指针（槽 0 = 数据指针、槽 1 = vtable 指针）。**调用 `check_method_call` Dyn 分支**：`let __data = FieldGet(obj,0); let __vtp = FieldGet(obj,1); let __m = Index(__vtp, 3+idx); CallIndirect(__m, [__data, ...实参])`——同一签名分派到不同 impl（多态）。MVP 限制：trait/impl 均须非泛型（泛型实例化报 Unsupported）；trait 方法签名含 `Self`（关联返回类型/参数）不支持经 dyn 调用；签名参数与返回类型须为具体类型。产出 `tests/run-pass/dyn_trait.{zeta,out}`（6 输出：转换/多态分派/胖指针拷贝/带参方法）+ 全量 40 用例全绿 + cargo test 全绿。
- [x] **G3 裸指针 / G4 生命周期标注**（2026-08）：
  - **G3 裸指针**：`*const T`/`*mut T` 类型注解（parser `ty.rs` 新增 `Token::Star` 分支；AST `AstType::RawPtr(Box<AstType>, bool)`；typecheck `Type::RawPtr(Box<Type>, bool)`；`resolve_ast_type` 分支）+ `*p` 读写（Deref 分支扩展 `RawPtr`，标量 load/store 与 `&T` 同构，codegen 同为 `i8*` 槽零改动）+ 引用 ↔ 裸指针**互视宽松规则**（`compatible_with` 双向：`(RawPtr, Ref)` 与 `(Ref, RawPtr)` 内层兼容即可——FFI 传参 `fn f(p: *const T)` 传 `&x`、`let p: *const T = &x;`，codegen 布局一致零成本）+ `*mut T` 降级 `*const T`（反向禁止）。接线：`field_scalar_of` → Ptr、`type_mono_key` `rawptr{const,mut}_`、`Display`、zeta-fmt/zeta-doc `fmt_type` 补分支。**关键教训**：参数检查 `ty.compatible_with(pty)` 是**实参×形参**方向，与 let 声明 `at.compatible_with(&ty)` 相反——单方向规则（RawPtr 接受 Ref）会在函数调用处失效，MVP 采宽松互视（双向）解决，借用安全性留给 borrowck。聚合裸指针访问字段需显式 `(*p).field`（不自动剥层）。产出 `tests/run-pass/raw_ptr.{zeta,out}`（7 输出：读取/写入/降级/传参/聚合显式解引用/函数返回值）。
  - **G4 生命周期标注（MVP 语法接受）**：parser 两处——`parse_generics` 遇 `Token::Lifetime` 跳过（`'a` 及可选 `: 'b` bound），`parse_type` 的 `&` 分支跳过 `&'a T` 生命周期（其后可选 mut）；typecheck 无改动（生命周期信息解析后丢弃，宽松检查）。grammar.md EBNF 早已定义（`Lifetime ::= ''' Ident`）；lexer 早有 `Token::Lifetime`。borrowck 生命周期检查规划中。产出 `tests/run-pass/lifetime.{zeta,out}`（4 输出：`fn longest<'a>(&'a i64, &'a i64) -> &'a i64`、`struct Wrapper<'a>`、`&'a mut T` 写入、与裸指针组合）。
- [x] **H3 捕获闭包（IIFE MVP）**（2026-08）：`(|x, y| body)(args)` 立即调用——闭包体引用的外层变量**按值捕获**，desugar 为匿名函数 `__closure_N(cap1, cap2, x, y) -> ret { body }`（捕获变量作前置参数、参数名保留外层原名；闭包参数无类型注解，类型从实参推断）+ 调用点展开 `__closure_N(cap1, cap2, arg1, arg2)`，零新增 IR 节点。实现：`check_call` 的 `_ =>` 分支拦截 `ExprKind::Closure` callee → `check_capture_closure_iife`；**捕获收集采用迭代重查法**（环境 = 已发现捕获变量 + 闭包参数，逐轮把 `UndefinedVariable` 中存在于外层变量环境的名称收为捕获并重查，循环收敛）；字符串字面量实参经 `check_string_from` 升级为 String 语义（与 `let s = "..."` 绑定一致，槽数匹配——否则闭包体内拼接按 `Str` 报 ExpectedNumeric，而字面量绑定变量本就为 `Str` 类型，属语言现状）。MVP 约束：仅 IIFE（`let f = |x| ..; f(..)` 闭包值对象 = 匿名结构体 + `call` 方法，规划中）；`move` 关键字 MVP 忽略（所有权宽松）；嵌套捕获闭包不支持；参数遮蔽捕获（Rust 语义）。H2 无捕获闭包不受影响（IIFE 无捕获时走同路径，结果一致）。产出 `tests/run-pass/capture_closure.{zeta,out}`（8 输出：单/多捕获、String 拼接、H2 回归、参数遮蔽、move、数组变量索引、for 循环内 IIFE）+ 全量 39 用例全绿 + cargo test 全绿。
- [x] **K4 `Gc<T>` 追踪 GC（MVP 保守标记-清除）**（2026-08）：编译器内建 + 独立运行时 crate `zeta-gc-runtime`。**布局**：`Gc<T>` 栈上 1 槽（Ptr）指向堆 1-槽包装（`Alloc{slots:1}`，槽 0 存 `GcInner` 基址）；对象 = `slot_count(T)` 个 8 字节槽连续堆块，`T` 值区自堆首槽起（与 `Box<T>` 完全同构，解引用/剥层零差异）。**生命周期协议**：`gc_region` 块 desugar 为 `zeta_gc_region_begin()`（`epoch += 1`）→ `zeta_gc_alloc(n)`（malloc 对象 + 注册块表 + 记录 epoch）→ `zeta_gc_escape(ptr)`（块返回值登记逃逸 root，空指针空操作）→ `zeta_gc_collect()`（从逃逸 root 标记 → 清除 `epoch` 匹配未标记对象 → 存活对象提升为 root → `epoch -= 1`）；块外对象 epoch 恒小于任何块 → 永不回收（MVP 泄漏语义）；跨块存活引用链经"存活提升"连续保护（内层逃逸对象在后续块 collect 中为 root）。**内建 API**：`Gc::new`（`check_gc_new`：alloc + 值区写 + 1-槽包装 + `FieldSet` 存 base）、`gc_region` 块（`check_gc_region`：body 语句 + final_expr 绑定 `__gc_escape` + escape 调用 + collect 调用 + 块表达式类型）、字段/方法/索引自动剥层（`heap_ptr_hir` 统一 Box/Rc/Arc/Gc）。**关键教训**：① `zeta_gc_escape` 参数必须传**对象 base**（`heap_ptr_hir` 解包装槽 0）——初版传 `Variable(esc)`（包装指针）导致 `mark` 线性查找（`header.base == root.base`）失配、对象被误回收（悬垂读取）；② `collect` 存活提升条件须为**所有被标记对象**（`marked`）而非 `marked && epoch == s.epoch`——嵌套块 collect 释放全部旧 root 节点，仅提升本块对象会使外层逃逸对象（`epoch < s.epoch` 存活但未标记可达）失去保护，在后续外层 collect 中因未标记且 epoch 匹配被误回收；③ **调试提醒**：`zeta run`/`build` 增量缓存命中会跳过 typecheck（源码哈希未变复用旧 LLVM IR），改编译器代码后必须 `--force`。**运行时加固**：全部动态内存只用 `libc::malloc`/`free`（对象块 + header/root 元数据链表），不用 Rust 堆分配与 `realloc`——macOS C 主程序环境实测 `RawVec`（`Vec::with_capacity`/`push` 扩容）与 `libc::realloc` 在 `libc::malloc` 之后调用触发 `libsystem_malloc` 的 `mfm_alloc` 崩溃（`_os_unfair_lock_unowned_abort` / SIGKILL，复现实验见 `zeta-gc-runtime` 模块头注释）；全局状态 `SyncUnsafeCell` 单线程无锁（MVP 编译产物单线程串行调用约定，无 Mutex）。产出 `tests/run-pass/gc_region.{zeta,out}`（15 输出：块内分配+解引用、逃逸对象跨块存活、字段/方法/索引剥层、嵌套块存活链式提升、循环重复收集、Gc 字段剥层、块外泄漏语义）+ `tests/compile-fail/gc_bad.zeta`（`Gc::new()` 参数个数断言）+ 全量 36 用例全绿 + cargo test 全绿。
- [x] **K3 `Rc<T>`/`Arc<T>` 引用计数装箱**（2026-08）：编译器内建（typecheck 特判，零新增 HIR/MIR/LIR/codegen 节点）。**布局**（MVP 注记，与 memory-model.md §4 顺序差异见该文档）：堆 `RcInner` 的 `T` 值区自堆首槽起（槽 0..`n`，`n = slot_count(T)`，与 `Box<T>` 同构，解引用/剥层零差异），尾部两计数槽：槽 `n` = strong_count、槽 `n + 1` = weak_count；`Rc<T>` 栈上 1 槽（Ptr）指向 RcInner，分配 `(n + 2)` 个 8 字节槽。内建 API：`Rc::new`/`Arc::new`（值区写后 `FieldSet` GEP 写计数 1/0——**关键教训：`DerefSet` base 是"地址"而 `FieldGet` 是 load 槽值，计数/值区写必须用 `FieldSet`（GEP + store），不能经 `FieldGet` 取地址**）、`clone`（强计数 +1 指针共享）、`strong_count`/`weak_count`（读计数槽，返回 `Type::USize`）、`downgrade`（弱计数 +1 → `Weak<T>`）、`try_unwrap`（强计数 == 1 → `Ok(T)` 解出值区 / 否则 `Err(Rc<T>)`，HIR 直接构造 Result 枚举对象）、`Weak::upgrade`（强计数 > 0 → 强计数 +1 返回 `Some(Rc<T>)` 否则 `None`，HIR `If` 构造 Option 枚举对象）。接线：`check_rc_method` 在 receiver 推断后、`heap_ptr_hir` 改写前分派（内建需原始 Rc 对象）；`heap_ptr_hir` 统一 Box/Rc/Arc（槽 0 即值区首槽，Rc 分支与 Box 合并）；`peel_refs_and_heap` 剥层；`Rc::new`/`Arc::new`/`Weak::upgrade` 静态路径特判（check_call `split_once("::")` 块）；Deref 分支扩展 Rc/Arc。调试收获：`zeta run` 缓存 key 不含编译器版本，改编译器后必须 `--force`；`try_unwrap` 首版 else 分支漏写 Err payload 槽致读未初始化内存崩溃（match `Err(rc)` 绑定读 payload）。产出 `tests/run-pass/rc_new.{zeta,out}`（17 输出：标量/方法/索引/字段剥层、clone 计数、弱引用升级、try_unwrap Err 分支、Arc 同构、Rc<Vec> push）+ `tests/compile-pass/rc_ty.zeta` + `tests/compile-fail/rc_bad.zeta` + 全量 33 用例全绿 + cargo test 全绿 + clippy 0 警告。
- [x] **K2 `Box<T>` 堆分配装箱**（2026-08）：`Box::new(v)` 编译器内建（无 std 结构体定义，纯 typecheck 特判）+ `*` 解引用 + 字段/方法/索引自动剥层。布局：**`Box<T>` 栈上 1 槽（Ptr）存堆指针，堆上分配 `slot_count(T)` 个 8 字节槽的连续对象区**（与对象槽区布局同构）——`Box::new` desugar 为 `alloc_bytes(8*n)` + 写入（标量 T 经 `HirExpr::DerefSet` 直接写堆首槽，复用 G1 解引用写链路；聚合 T 经 `array_copy` 整槽区 memcpy，浅拷贝与 MVP 结构体赋值一致）+ 1 槽 `Alloc` 对象（槽 0 = 堆指针）。核心新增：`check_box_new`（参数检查 + `type_slot_count` 槽数计算：标量 1 / struct 字段数 / enum `slot_count` / 数组长度 / 元组元素数 / 引用·Fn·内嵌 Box 1 槽 / 动态数组 1 指针槽）+ `peel_box`/`peel_refs_and_boxes`/`box_ptr_hir`（`Box<T>` 表达式 → 堆对象指针 `FieldGet(box, 0, Ptr)`）。接线五处：① `UnaryOp::Deref` 加 `Box<T>` 分支（标量 load / 聚合指针拷贝，与 `&T` 同构）；② 字段访问 `peel_refs_and_boxes` + base 改写；③ 方法调用 receiver 改写（`Box<String>` 的 len/索引、`Box<Vec<i64>>` 的 push 均可，`&self` 收到 `T` 对象指针）；④ 索引访问 base 改写；⑤ `as_str` 特判支持 `Box<String>`。`Vec::with_capacity` 无上下文返回 `Vec<Infer>`，测试经 `Box<Vec<i64>>` 注解统一。嵌套 `Box<Box<i64>>`（`**bb`）与 Box 赋值指针共享（浅拷贝）可用。产出 `tests/run-pass/box_new.{zeta,out}`（15 输出：标量/浮点/布尔解引用、类型注解、嵌套、String 方法+索引、struct 字段+聚合解引用、Vec push+索引、指针共享）+ `tests/compile-pass/box_ty.zeta`（函数参数/返回值注解）+ `tests/compile-fail/box_bad.zeta`（`Box::new()` 参数个数断言）+ 全量 30 用例全绿 + cargo test 全绿 + clippy 0 警告。
- [x] **J1–J3 迭代器与集合协议**（2026-08）：
  - **J1 数组迭代**：`for x in arr`——typecheck 新增 `check_for_array`（数组以指针存储、长度编译期已知，desugar 为 `let __for_arr = <数组>; let mut __for_i = 0; loop { if __for_i >= N { break; } let pat = __for_arr[__for_i]; __for_i += 1; body }`，元素读取复用 `HirExpr::Index` 步长 8 / u8 按字节；元素类型 Infer 报 Unsupported）。`check_for` 分派加 `Type::Array` 分支。
  - **J2 自定义迭代器接入 for**：接收者类型存在 `next() -> Option<Item>` 方法（inherent / trait impl，`find_impl_for_method` + `unify` 替换）时，构造 AST `let mut __for_it = it; loop { match __for_it.next() { Some(__elem) => { let pat = __elem; body }, None => break } }` 交 `check_for_iterator` infer（复用 check_method_call / check_match 全链路）。配套修复：parser `stmt_terminator` 补 `,`（`match { None => break, }` 臂体解析）。测试 `tests/run-pass/iterator_for.{zeta,out}`（inherent + trait 迭代器 / 手动 next / 按值绑定，6 输出）。
  - **J3 适配器**：`map`/`filter`/`fold`/`collect`/`take`/`skip` 内建 desugar（`check_method_call` 入口特判，接收者为数组 / `Vec<T>` / 自定义迭代器时启用）：新增 `closure_return_ty`（闭包体实际返回类型预推断，U 用于收集容器类型注解，避免 `Vec<Infer>` 被 for 拒绝）、`ty_to_ast`（Type → AstType 注解）、`try_check_adapter`/`check_iterator_adapter`。desugar 结构：闭包经 `check_closure_expected` 按 `fn(T...) -> U` 签名注入匿名函数（H2，返回函数指针 `__adp_f`）；收集容器 `let mut __out: Vec<U> = Vec::new();`（带注解）；循环复用现有分派——数组/Vec 走 `for __x in ..`（J1/check_for_vec），迭代器走 `loop { match it.next() { Some(__x) => apply, None => break } }`。apply：map `let __u = __f(__x); __out.push(__u);` / filter `if __f(__x) { push }` / fold `__acc = __f(__acc, __x);`（返回 acc）/ take `if __n < n { __n += 1; push } else { break }` / skip `if __n < n { __n += 1 } else { push }` / collect `push`。适配器结果 Vec 亦可作下一适配器源（链式）。产出 `tests/run-pass/adapters.{zeta,out}`（13 输出：数组/Vec/迭代器 × 6 适配器 + 链式）+ `tests/compile-fail/adapter-badargs.zeta`（非闭包参数断言）+ 全量 27 用例全绿 + cargo test 全绿 + clippy 0 警告。
- [x] **K1 `?` 运算符**（2026-08）：`expr?` 在 Option/Result 上下文 desugar 为 `match` + `return` 早返回（零新增 HIR 节点，复用 check_match 的 if-else 链 + tag 比较）。核心：① 拆分 `check_match_with_scrutinee`（接受调用方已推断的 scrutinee HIR + 类型，避免 inner 内嵌闭包重复 desugar）；② `check_question`（typecheck）——infer inner → `Type::Named("Option"/"Result")` 特判 → 构造 AST MatchArm（成功臂 `Some(__v) => __v` / 失败臂 `None => return Option::None`、`Err(__e) => return Result::Err(__e)`，失败变体经完整路径构造）+ `check_match_with_scrutinee` 执行；非 Option/Result 类型报 Unsupported。③ 裸无参变体值表达式支持：infer_expr Ident 分支补 `split_variant_path` 兜底（`return None;` 可用）。接线：parser 后缀循环 `?`（AST `ExprKind::Question`）；zeta-fmt（PREC_POSTFIX 后缀重建）/zeta-check（walk_expr 补分支）。MVP 约束：返回类型兼容性检查与现有 `return` 语义一致（宽松）。产出 `tests/run-pass/question.{zeta,out}`（5 输出，含表达式中间嵌套双 `?`）+ `tests/compile-pass/question_result.zeta`（Result 解包）+ `tests/compile-fail/question-nonoption.zeta`（非 Option/Result 报错断言）+ 全量 23 用例全绿 + clippy 0 警告。
- [x] **H2 无捕获闭包**（2026-08）：`|x, y| expr` desugar 为匿名函数（`__closure_N`）+ 函数指针，零运行时开销。核心：`check_closure_expected`（typecheck）——按预期 fn 签名取参数类型、`std::mem::take` 清空变量环境实现无捕获隔离（body 引用外部变量 → `UndefinedVariable` 改写为 Unsupported「闭包捕获外部变量（H3 规划）」）、body 尾部表达式兼容返回类型、注册 `fn_signatures` + `ctx.mono_items` 注入 `HirItem::Fn`（复用 H1 函数指针全链路）。接线三处：`check_call` / `check_indirect_call` 实参循环（形参 `Type::Fn` + 实参闭包 → 预期签名检查）+ `check_stmt` Let 分支（fn 注解 + 闭包 init）。MVP 约束：参数无类型注解需 fn 上下文、仅标识符/`_` 参数模式、不支持返回闭包的函数（`let f: fn(..) = |..| ..; f` 转接）、`move` 语义规划。产出 `tests/run-pass/closure.{zeta,out}`（7 输出）+ `tests/compile-pass/closure.zeta` + `tests/compile-fail/closure-capture.zeta` + 全量 20 用例全绿 + clippy 0 警告。
- [x] **H1 函数类型与函数指针**（2026-08）：`fn(A) -> B` 函数类型 + 函数一等值全链路（parser → typecheck `Type::Fn` 签名 + `infer_expr` 函数值推断 + `check_indirect_call` 兜底 → HIR `HirExpr::FnPtr`/`CallIndirect` → MIR/LIR `CallIndirect` → LLVM：函数指针统一存 `i8*` 槽，存储前按签名 `bitcast i64(...)* @fn to i8*`，调用时 `bitcast` 回 `{ret}({params})*` 后间接 `call`）。函数值支持作实参、作返回值（`choose`）、重新绑定、显式类型注解。**关键修复**：① MIR DCE 活跃性收集漏算 `CallIndirect` 实参（实参临时赋值被误删 → 未初始化栈读取垃圾值）；② MIR inline pass 缺失 `CallIndirect` 分支（内联后整个间接调用被丢弃，返回值临时从未写入）；③ borrowck/regionck 补 `FnPtr`/`CallIndirect` 分支；④ `mir_lower_test.rs` match 补 `call_indirect` arm。产出 `tests/compile-pass/fn_ptr.zeta` + `tests/run-pass/fn_ptr.{zeta,out}` + 全量 17 用例全绿 + clippy 0 警告。
- [x] **G2 `str` 切片与 String 补齐**（2026-08）：`&str` 引用切片 + `String::from` 运行期内容。**块一**：`String::from(runtime_string)` desugar 为 `s.clone()`（标准库深拷贝，运行期读 len 槽——消除 §13 约束 4 的"长度表达未实现"；签名相同的重声明不受影响）。**块二**：`&str` 视图（`&str` 运行时 = 指向 String 对象的瘦指针，复用 G1 聚合引用机制，MIR/LIR/codegen 布局零改动）：`String::as_str()` 特判 desugar 为 `HirExpr::Ref`（对象指针拷贝）返回 `Ref(Str)`；`resolve_ast_type` 识别 `str` 类型关键字；方法调用把 `Ref(Str)` 接收者归一为 String impl（len/substring 等）；`check_index`/`check_slice` 对 `&str` 先取 data 槽（同 String 对象）；`String::from(&str)` 读 data/len 槽深拷贝（alloc_bytes + copy_bytes + 三槽）；`compatible_with` 允许 `&str` ↔ `&String` 互视 + 视图与 String 值比较互用；`comparison.rs` 字符串内容比较纳入 `&str`。**设计决策**：计划原文"substring 返回引用而非拷贝"改为**兼容方案**——`substring` 保持 String 拷贝返回（避免破坏大量现有调用者与文档 API），零拷贝切片以新增 `as_str()` 借用视图体现。产出 `string_from_runtime_test.rs` 6 用例 + `str_ref_test.rs` 9 用例 + 全量 114 套件回归绿 + clippy 0 警告。
- [x] **G1 引用类型与表达式**（2026-08）：`&T`/`&mut T` 类型 + `&x`/`&mut x`/`*` 表达式全链路打通（parser → typecheck → HIR → MIR → LIR → LLVM）。核心设计：**标量引用存 `i8*` 值槽、聚合引用存对象指针拷贝**；`*p` 读取/写入时按标量种类 `bitcast i8*` 转换；字段访问/方法调用自动 `peel_ref`（`p.x`/`v.len()` 免显式解引用）；`&mut T` 兼容 `&T` 参数（宽松规则，严格可变性互斥留给 borrowck）。关键修复：① MIR inline 补 `AddrOf`/`DerefRead`/`DerefWrite` 三指令映射（否则内联丢弃引用指令、标量读取输出垃圾）；② MIR DCE 活跃性分析纳入 Deref* 读操作（`*p = *p + 1` 的临时变量免被删）；③ codegen `operate_value` 与 `store_to` 指针拷贝避免 `%%` 转义；④ **顶层同名遮蔽**：用户顶层 `fn read` 与 std extern `read` 重名时，用户侧声明注册为 `read@shadow<N>` mangle 名（仅签名不同时），std 原名保留——std 模块（如 `io.zeta` 内部 `read(0, tmp, 256)`）仍绑定 extern 原版，用户顶层代码经 `fn_shadow_of` 绑定自身版本，下游符号名唯一不冲突；签名相同的重声明（如 `extern fn __zeta_target_os()`）视为无害重声明保留原名。产出 `reference_test.rs` 10 用例（标量读写/复合赋值、struct 读写、vec 方法、引用链、别名拷贝、返回引用、`&mut`→`&` 传递）+ 全量 112 套件回归绿。

---

## 5. 跟踪与验收约定

1. 每个阶段/任务完成须满足：`cargo test --workspace` 全绿 + `cargo clippy --workspace --all-targets` 0 警告。
2. 涉及运行时/并发类测试（Actor、通道、锁、join、GC 周期）须按 `design/00_项目总览.md` 硬性规则带超时保护，挂起即视为失败。
3. 任务完成后同步更新三处：本文档状态标识 + `CODEBUDDY.md`（§3.9 限制速览勾销 / §5.5 执行记录）+ `guide.md` §13（勾销对应限制条目，并更新 `grammar.md` 顶部实现状态标注）。
4. 每个阶段产出对应的集成测试（仿 development-plan.md 各阶段 `*_test.rs` 用例并全量回归）。
5. 优先交付顺序建议：G1/G2（引用地基 + str）→ I1/I2（宏 + 格式化）→ H1/H2（函数指针 + 无捕获闭包）→ K1（`?`）→ J 全阶段 → 其余。

---

> **维护者**：Zeta Language Team
> **最后更新**：2026-08-22
