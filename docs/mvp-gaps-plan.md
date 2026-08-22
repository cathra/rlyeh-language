# 开发计划 — MVP 已知限制消解（阶段 G–L）

> **性质**：本文档为 [`guide.md`](./guide.md) §13「已知限制（MVP）」的消解计划，承接 [`development-plan.md`](./development-plan.md)（阶段 A–F 全部完成后）的续作，为**第三版规划的权威副本**。
> **需求来源**：`docs/guide.md` §13 已知限制（MVP）列出的 7 项规划特性 + 5 项实现约束。
> **总纲与进度速览**：见根目录 [`CODEBUDDY.md`](../CODEBUDDY.md)（§3.8 限制速览表、§5.5 执行记录）。
> **维护规则**：每完成一项任务，需同步更新本文档状态标识 + `CODEBUDDY.md` §3.8 / §5.5 + `guide.md` §13（勾销对应限制条目）。

---

## 1. 背景：MVP 已知限制盘点

### 1.1 规划中 / 未实现特性（7 项）

| # | 限制（guide.md §13） | 编译器现状依据 | 归属阶段 |
|---|---------------------|---------------|---------|
| 1 | **宏系统**：`println!` / `vec!` / `format!` 等宏调用不支持（`!` 是 `not` 一元运算符）；打印用内建 `println(expr)`（0–1 参数，无 `{}` 格式化） | grammar.md §2.14 已有 `macro_rules!` EBNF（规划标注）；typecheck `check_call` 已有 `name.ends_with('!')` 分支；std-lib.md §8 `Display`/`Debug`/`format!` 为规划 API | **I** |
| 2 | **引用与借用**：`&x` 表达式、`&T` 参数类型、`str` 类型、解引用 `*`、裸指针均未实现；仅方法接收者 `&self`/`&mut self` 可用 | grammar.md Type 规则含 `'&' Lifetime? 'mut'? Type`、UnaryExpr 含 `'*' | '&' 'mut'?`、Pattern 含 `'ref'`（均已定义未实现）；typecheck `UnaryOp::Deref/AddrOf/AddrOfMut` → Unsupported（check_expr.rs）；borrowck crate 仅服务 `&self` 接收者 | **G** |
| 3 | **闭包**：`|x| x + 1` 语法可解析，typecheck 报 Unsupported | parser 已产出 `AstExpr::Closure`；typecheck 报 Unsupported（check_expr.rs） | **H** |
| 4 | **运算符**：`?` 错误传播、`dyn Trait`、函数指针未实现 | `AstType::Fn(_, _)` → Unsupported（check_expr.rs）；grammar.md 含 `'dyn' TraitBound` 与后缀 `'?'`（已定义未实现）；`Option`/`Result` + `expect`/`unwrap_or` 已实现 | `?`→**K**，函数指针/dyn→**H** |
| 5 | **所有权层级**：L2 `Rc<T>`/`Arc<T>`、L3 `Gc<T>` 未实现 | memory-model.md §4（Rc/Arc）/§5（Gc）规范完备（含布局、转换矩阵、开销表）；`Box<T>` 亦为 §3 目标 API | **K** |
| 6 | **并发**：`serde`/`fmt`/`async` 模块为规划；actor 的 `async` 方法 + `.await`/`send` 已实现 | std-lib.md §8（fmt）/§9（serde）/§10（async 运行时）规划标注；actor 异步为独立机制 | fmt→**I**，serde/async→**L** |
| 7 | **迭代器协议**：数值区间、`for x in vec`/`for (k, v) in map` 可用；**数组迭代不支持**；`Iterator` trait/`collect` 未实现 | typecheck for 循环三路分支（range/Vec/HashMap），其余类型 → Unsupported（check_expr.rs）；std-lib.md §2.3 Iterator 规划 | **J** |

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
| **G** | 引用与借用（L0 完整化） | `&T`/`&mut T`、`*` 解引用、`str` 切片、裸指针、生命周期 `'a` | 无（地基） | ⏳ 未开始 |
| **H** | 一等函数 | `fn(A) -> B` 函数类型、闭包（捕获 + `move`）、`dyn Trait` | G（引用捕获） | ⏳ 未开始 |
| **I** | 宏系统与格式化 | `macro_rules!` 声明式宏、`Display`/`Debug`、`println!`/`format!` | 弱（可与 G/H 并行） | ⏳ 未开始 |
| **J** | 迭代器与集合协议 | 数组迭代、`Iterator` trait、`map`/`filter`/`fold`/`collect` | H（适配器闭包） | ⏳ 未开始 |
| **K** | 错误传播与所有权层级 | `?` 运算符、`Box<T>`、`Rc<T>`/`Arc<T>`、`Gc<T>` | G（指针操作） | ⏳ 未开始 |
| **L** | 生态模块与平台收尾 | `async fn`/`await`、`serde`、region `strategy (bump)`、WASI net / Actor 交叉编译 | I（serde 宏）、G 等 | ⏳ 未开始 |

> **推荐执行路线**：
> - **保守路线（依赖驱动）**：G → H → J → K → L，I 按需插入（宏展开器在 parse 后、typecheck 前，与 G/H 解耦，可随时并行）。
> - **快赢路线（体验驱动）**：先做 I1/I2（宏 + `println!` 格式化，开发者日常收益最大、不依赖引用系统）与 G 同步推进；J1（数组迭代）独立且极小，可先行交付。
> - 优先级建议：G1–G2（引用地基 + str）> I1–I2（宏 + 格式化）> H1–H2（函数指针 + 无捕获闭包）> K1（`?` 运算符）> J 全阶段 > 其余。

---

## 3. 阶段详情

### 阶段 G — 引用与借用（L0 完整化）

> 现状：`&x`/`&mut x`/`*` 已实现（G1 ✅）：`&T`/`&mut T` 类型、取址/解引用表达式、参数与返回值引用、标量与聚合引用内存模型、字段访问/方法调用的自动 `peel_ref`、借用宽松规则（`&mut T` 兼容 `&T` 参数，严格可变性互斥留给 borrowck）均可用；`&self`/`&mut self` 方法接收者保持可用；`&str` 视图已实现（G2 ✅）：`String::as_str()` 只读借用（瘦指针，运行时 = 指向 String 对象的指针）、`&str` 参数/返回、字节索引、`String::from(&str)` 深拷贝、视图与 String 内容比较互用；`String::from(String)` 运行期长度表达可用。borrowck crate 存在但仅服务接收者。`ref` 模式、严格借用检查与 `strategy (bump)` 等仍规划中。

| 任务 | 内容 | 状态 |
|------|------|------|
| G1 | **引用类型与表达式**：`&T`/`&mut T` 类型、`&x`/`&mut x` 表达式、`*` 解引用；函数参数 `x: &T`、返回 `&T`；`ref` 模式。typecheck 类型规则 + borrowck 借用规则（可变性、悬垂、别名）接线 | ✅ |
| G2 | **`str` 切片与 String 补齐**：`&str` 引用切片（新增 `as_str()` 只读借用视图，`substring` 保持拷贝返回以免破坏现有 API）；`String::from(s)` 支持运行期 String/str 内容（长度表达），消除 §13 约束 4 | ✅ |
| G3 | **裸指针**：`*const T`/`*mut T` 类型 + `*p` 读写（FFI 场景；codegen 直通） | ⏳ |
| G4 | **生命周期标注**：`'a` 参数（grammar.md 已定义）、省略规则；borrowck 生命周期检查 | ⏳ |

**风险与对策**：借用检查是全语言最复杂的子系统，建议分四步走——先允许（宽松借用，typecheck 报错兜底）→ 严格借用检查 → 生命周期 → 指针；每步以 `cargo test --workspace` + clippy 0 警告为准入。

### 阶段 H — 一等函数（闭包 + 函数指针）

> 现状：`|x| x + 1` 可解析（typecheck 报 Unsupported）；`AstType::Fn(_, _)` 报 Unsupported；trait/impl 泛型单态化已实现，可为函数类型复用。

| 任务 | 内容 | 状态 |
|------|------|------|
| H1 | **函数类型与函数指针**：`fn(A) -> B` 类型（含泛型实例化）、函数作为值传递与调用（codegen 函数指针） | ⏳ |
| H2 | **无捕获闭包**：`|x| expr` desugar 为普通函数 + 函数指针（零开销、无依赖 G） | ⏳ |
| H3 | **捕获闭包**：按值捕获 desugar 为匿名结构体（捕获字段 + `call` 方法）；`move` 语义；按引用捕获依赖 G | ⏳ |
| H4 | **`dyn Trait`**：trait 对象（数据指针 + vtable：drop/size/align/方法表），`dyn Trait` 类型 + 强制转换 | ⏳ |

### 阶段 I — 宏系统与格式化

> 现状：grammar.md §2.14 已有 `macro_rules!` EBNF（规划标注）；typecheck `check_call` 对 `name.ends_with('!')` 已有宏调用分支（但未实现展开）；std-lib.md §8 的 `Display`/`Debug`/`println!`/`format!` 为规划 API。**与 G/H 解耦，可并行。**

| 任务 | 内容 | 状态 |
|------|------|------|
| I1 | **宏调用语法区分**：词法/语法层区分 `name!`（宏调用）与 `!`（not 一元运算符）；`macro_rules!` 声明式宏（matcher → transcriber，token 流展开于 parse 后、typecheck 前） | ⏳ |
| I2 | **`Display`/`Debug` trait + 格式化宏**：`{}` 占位符格式化引擎；`println!`/`print!`/`format!`/`dbg!` 宏；String 拼接语义（`{}` 插入 `to_string`） | ⏳ |
| I3 | **集合宏**：`vec!`/`map!`/`arr!`（展开为 `Vec::new` + push / `HashMap::insert` 序列） | ⏳ |

### 阶段 J — 迭代器与集合协议

> 现状：for 循环三路分支（数值区间 / Vec / HashMap）已实现，其余类型报 Unsupported；std-lib.md §2.3 `Iterator` trait 为规划 API。

| 任务 | 内容 | 状态 |
|------|------|------|
| J1 | **数组迭代**：`for x in arr`（desugar 为数组下标循环，数组切片模式可复用） | ⏳ |
| J2 | **`Iterator` trait**：`next() -> Option<Item>` 核心方法；自定义迭代器接入 `for`（经 trait 方法调用） | ⏳ |
| J3 | **适配器**：`map`/`filter`/`fold`/`collect`/`take`/`skip`（依赖 H 闭包） | ⏳ |

### 阶段 K — 错误传播与所有权层级

> 现状：`Option`/`Result` + `expect`/`unwrap_or` 已实现（`?` 缺失）；memory-model.md §4（Rc/Arc）/§5（Gc）规范完备（布局/转换矩阵/开销表）；`Box<T>` 为 §3 目标 API。

| 任务 | 内容 | 状态 |
|------|------|------|
| K1 | **`?` 运算符**：Result/Option 早返回 desugar（`match` + `return`），要求函数返回兼容类型 | ⏳ |
| K2 | **`Box<T>`**：堆分配目标 API（`Box::new`；解引用依赖 G） | ⏳ |
| K3 | **`Rc<T>`/`Arc<T>`**：原子/非原子引用计数（memory-model.md §4 布局；`Rc::new`/`clone`/`try_unwrap`；`Deref` 特判或依赖 G） | ⏳ |
| K4 | **`Gc<T>`（插件式）**：标记-清除 GC + `gc_region`（memory-model.md §5；write barrier、逃逸限制） | ⏳ |

### 阶段 L — 生态模块与平台收尾

| 任务 | 内容 | 状态 |
|------|------|------|
| L1 | **`async fn`/`await`**：普通函数异步支持（actor `async` 方法已有独立机制，抽象复用） | ⏳ |
| L2 | **`serde` 序列化模块**：`Serialize`/`Deserialize` trait + `#[derive]` 风格宏（依赖 I） | ⏳ |
| L3 | **region 选项接线**：`strategy (bump)` 等其余选项（`zeta-region-alloc` 已就绪，PGO 预测可直接回灌 `adaptive` 初始容量） | ⏳ |
| L4 | **平台加固**：WASI 下 net 支持（或明确禁用文档化）；Actor 交叉编译 / WASM 支持（消除 §13 约束 2/3） | ⏳ |

---

## 4. 执行记录

- [x] **G2 `str` 切片与 String 补齐**（2026-08）：`&str` 引用切片 + `String::from` 运行期内容。**块一**：`String::from(runtime_string)` desugar 为 `s.clone()`（标准库深拷贝，运行期读 len 槽——消除 §13 约束 4 的"长度表达未实现"；签名相同的重声明不受影响）。**块二**：`&str` 视图（`&str` 运行时 = 指向 String 对象的瘦指针，复用 G1 聚合引用机制，MIR/LIR/codegen 布局零改动）：`String::as_str()` 特判 desugar 为 `HirExpr::Ref`（对象指针拷贝）返回 `Ref(Str)`；`resolve_ast_type` 识别 `str` 类型关键字；方法调用把 `Ref(Str)` 接收者归一为 String impl（len/substring 等）；`check_index`/`check_slice` 对 `&str` 先取 data 槽（同 String 对象）；`String::from(&str)` 读 data/len 槽深拷贝（alloc_bytes + copy_bytes + 三槽）；`compatible_with` 允许 `&str` ↔ `&String` 互视 + 视图与 String 值比较互用；`comparison.rs` 字符串内容比较纳入 `&str`。**设计决策**：计划原文"substring 返回引用而非拷贝"改为**兼容方案**——`substring` 保持 String 拷贝返回（避免破坏大量现有调用者与文档 API），零拷贝切片以新增 `as_str()` 借用视图体现。产出 `string_from_runtime_test.rs` 6 用例 + `str_ref_test.rs` 9 用例 + 全量 114 套件回归绿 + clippy 0 警告。
- [x] **G1 引用类型与表达式**（2026-08）：`&T`/`&mut T` 类型 + `&x`/`&mut x`/`*` 表达式全链路打通（parser → typecheck → HIR → MIR → LIR → LLVM）。核心设计：**标量引用存 `i8*` 值槽、聚合引用存对象指针拷贝**；`*p` 读取/写入时按标量种类 `bitcast i8*` 转换；字段访问/方法调用自动 `peel_ref`（`p.x`/`v.len()` 免显式解引用）；`&mut T` 兼容 `&T` 参数（宽松规则，严格可变性互斥留给 borrowck）。关键修复：① MIR inline 补 `AddrOf`/`DerefRead`/`DerefWrite` 三指令映射（否则内联丢弃引用指令、标量读取输出垃圾）；② MIR DCE 活跃性分析纳入 Deref* 读操作（`*p = *p + 1` 的临时变量免被删）；③ codegen `operate_value` 与 `store_to` 指针拷贝避免 `%%` 转义；④ **顶层同名遮蔽**：用户顶层 `fn read` 与 std extern `read` 重名时，用户侧声明注册为 `read@shadow<N>` mangle 名（仅签名不同时），std 原名保留——std 模块（如 `io.zeta` 内部 `read(0, tmp, 256)`）仍绑定 extern 原版，用户顶层代码经 `fn_shadow_of` 绑定自身版本，下游符号名唯一不冲突；签名相同的重声明（如 `extern fn __zeta_target_os()`）视为无害重声明保留原名。产出 `reference_test.rs` 10 用例（标量读写/复合赋值、struct 读写、vec 方法、引用链、别名拷贝、返回引用、`&mut`→`&` 传递）+ 全量 112 套件回归绿。

---

## 5. 跟踪与验收约定

1. 每个阶段/任务完成须满足：`cargo test --workspace` 全绿 + `cargo clippy --workspace --all-targets` 0 警告。
2. 涉及运行时/并发类测试（Actor、通道、锁、join、GC 周期）须按 `design/00_项目总览.md` 硬性规则带超时保护，挂起即视为失败。
3. 任务完成后同步更新三处：本文档状态标识 + `CODEBUDDY.md`（§3.8 限制速览勾销 / §5.5 执行记录）+ `guide.md` §13（勾销对应限制条目，并更新 `grammar.md` 顶部实现状态标注）。
4. 每个阶段产出对应的集成测试（仿 development-plan.md 各阶段 `*_test.rs` 用例并全量回归）。
5. 优先交付顺序建议：G1/G2（引用地基 + str）→ I1/I2（宏 + 格式化）→ H1/H2（函数指针 + 无捕获闭包）→ K1（`?`）→ J 全阶段 → 其余。

---

> **维护者**：Zeta Language Team
> **最后更新**：2026-08-22
