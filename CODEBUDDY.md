# CODEBUDDY.md — Zeta 系统级编程语言

> **项目代号**：Zeta  
> **版本**：v2.0  
> **状态**：MVP 开发中  
> **目标平台**：Linux / macOS / Windows / WASM  
> **实现语言**：Rust（自举编译器，bootstrap 阶段用 Rust 实现）

---

## 1. 项目愿景

Zeta 是一门面向未来十年基础设施的**系统级编程语言**，设计目标：

| 目标 | 对标 |
|------|------|
| 内存安全、零 GC | Rust |
| 编译速度极快 | Go |
| 并发模型一等公民 | Erlang / Akka |
| 数学式语法直觉 | Python / MATLAB |
| 开发体验友好 | Go / Python |

**一句话定位**：Zeta = Rust 的安全性 + Go 的编译速度 + Erlang 的并发模型 + 数学的自然表达。

---

## 2. 项目结构

```
zeta-language/
├── CODEBUDDY.md              ← 本文件（项目总纲）
├── CHANGELOG.md              ← 版本变更记录
├── Cargo.toml                ← Rust 工作区配置
├── README.md                 ← 项目介绍
│
├── crates/                   ← 编译器各组件（Rust crate）
│   ├── zeta-lexer/           ← 词法分析器
│   ├── zeta-parser/          ← 语法分析器
│   ├── zeta-ast/             ← 抽象语法树定义
│   ├── zeta-hir/             ← 高级中间表示
│   ├── zeta-mir/             ← 中级中间表示
│   ├── zeta-lir/             ← 低级中间表示
│   ├── zeta-typecheck/       ← 类型检查器
│   ├── zeta-borrowck/        ← 借用检查器
│   ├── zeta-regionck/        ← 区域检查器
│   ├── zeta-codegen/         ← 代码生成（LLVM / Cranelift）
│   ├── zeta-driver/          ← 编译器驱动（CLI 入口）
│   ├── zeta-std/             ← 标准库（Zeta 源码）
│   ├── zeta-actor-runtime/   ← Actor 运行时
│   ├── zeta-region-alloc/    ← 区域分配器（bump + 智能）
│   └── zeta-lsp/             ← 语言服务器（LSP over stdio）
│
├── tools/                    ← 工具链
│   ├── zeta-fmt/             ← 代码格式化
│   ├── zeta-check/           ← Linter
│   ├── zeta-doc/             ← 文档生成
│   └── zeta-bench/           ← 基准测试框架
│
├── zep/                      ← 包管理器
│   ├── Cargo.toml
│   └── src/
│
├── docs/                     ← 权威规范 + 开发进度文档（导航见 docs/README.md）
│   ├── README.md             ← 文档导航（入口）
│   ├── guide.md              ← 语言教程（示例均可运行）
│   ├── grammar.md            ← 完整语法规范（EBNF，含规划标注）
│   ├── semantics.md          ← 语义规则（含规划标注）
│   ├── memory-model.md       ← 分层内存管理规范（含规划标注）
│   ├── actor-model.md        ← Actor 并发模型规范（含规划标注）
│   ├── module-system.md      ← 模块系统规范（语法/语义/编译模型，含规划标注）
│   ├── std-lib.md            ← 标准库 API 规范（已实现 / 规划）
│   ├── development-plan.md   ← 开发计划（阶段 A–F 执行记录，已完成）
│   ├── mvp-gaps-plan.md      ← 剩余任务消解（阶段 G–T，进行中）
│   └── design/               ← 早期设计稿归档（映射见 design/README.md）
│
├── examples/                 ← 示例代码
│   ├── hello-world.zeta
│   └── arith-print.zeta
│
├── tests/                    ← 集成测试（.zeta 用例；Rust 集成测试位于各 crate 的 tests/ 目录）
│   ├── compile-pass/
│   ├── compile-fail/
│   └── run-pass/
│
│   ├── design/prompts/       ← 开发任务书归档（P001–P013 已全部完成，内容并入各规范文档"附录 A：实现纪要"）
│
└── .github/                  ← CI/CD
    └── workflows/
        ├── ci.yml
        └── release.yml
```

---

## 3. 核心语言特性速览

### 3.1 分层内存管理

| 层级 | 机制 | 关键字 | 运行时开销 |
|------|------|--------|------------|
| L0 | 静态所有权 + 借用 + 生命周期 | `let`, `&`, `&mut` | 零开销 |
| L1 | 区域（Region） | `region 'r {}`, `in 'r`, `transfer` | 零开销（批量释放） |
| L2 | 引用计数 | `Rc<T>`, `Arc<T>` | 原子操作 |
| L3 | 可选 GC | `Gc<T>`（插件式） | GC 停顿 |

### 3.2 数学式条件判断

```zeta
// 区间内（正向比较链）
if 0 < x < 10 {}
if 0 <= x <= 10 {}
if 0 < x <= 10 {}

// 区间外（反向比较链）
if 0 > x > 10 {}    // x < 0 || x > 10
if 0 >= x >= 10 {}  // x <= 0 || x >= 10

// 集合判断（括号内为集合，范围元素展开为离散成员）
if x in (1, 3, 5) {}
if ch in ('a'..<'z', 'A'..<'Z') {}
if x in (0...10) {}  // 等价于 x == 0 || x == 1 || ... || x == 10
if x not in (0..<10) {}  // 等价于 x != 0 && x != 1 && ... && x != 9

// in 右侧裸范围 = 区间判断（与集合成员判断语义不同）
if x in 0..<10 {}  // 0 <= x < 10   [0, 10)
if x in 0...10 {}  // 0 <= x <= 10  [0, 10]
if x in 0<..10 {}  // 0 < x <= 10   (0, 10]

// 时间字面量（分钟单位：9am = 540，6pm = 1080；hour 取分钟值）
if hour in (9am...6pm) {}
if hour not in (6am..<10pm) {}  // 跨午夜
```

### 3.3 Actor 并发模型

```zeta
actor Counter {
    value: i64 = 0,

    // 方法返回 -1 会被 runtime 视为崩溃信号（Panic）
    pub fn increment(amount: i64) -> i64 {
        self.value += amount;
        self.value
    }
}

let counter = Counter::new();                 // 普通 spawn（无监督）
let result = counter.increment(10).await;     // ask 同步往返

send counter.increment(1);                    // fire-and-forget（异步）

// 受监督 spawn：0=OneForOne 1=AllForOne 2=RestartForOne，
// 崩溃后 runtime 经 __state_new 重建初始状态并重启
let supervised = Counter::new_supervised(0);
```

语义：actor 方法消息经「kind 槽 + 3 个 i64 消息槽」传递，同一 actor 消息按邮箱 FIFO 互斥处理；返回 -1 触发崩溃协议（ask 立即返回 0，supervisor 重启；无监督则 actor 停止）。

### 3.4 区域系统

```zeta
// 基本用法
region 'r {
    let data = BigStruct::new() in 'r;
    process(&data);
} // 批量释放

// 转移所有权到外部
fn create_data() -> BigStruct {
    region 'r {
        let data = BigStruct::new() in 'r;
        return transfer data out of 'r;
    }
}

// 智能分配（编译器自动推断大小，PGO 画像回灌初始容量）
region 'r adaptive {
    for i in 0..<10000 {
        let obj = Data::new(i) in 'r;
    }
}

// 精确预分配 / 显式 bump 策略（L3 ✅：region 指令接线 zeta-region-alloc C ABI 运行时）
region 's with_size (4096) {
    let buf = BigStruct::new() in 's;
}
region 't strategy (bump) {
    let p = Point { x: 1, y: 2 } in 't;
}
```

### 3.5 聚合对象与泛型

```zeta
// 枚举 + 匹配
enum Shape { Circle(f64), Rect { w: f64, h: f64 } }

fn area(s: Shape) -> f64 {
    match s {
        Shape::Circle(r) => 3.14 * r * r,
        Shape::Rect { w, h } => w * h,
    }
}

// trait + impl + 泛型（单态化）
trait Area { fn area(&self) -> f64; }
impl Area for Shape { fn area(&self) -> f64 { /* ... */ } }

// 结构体字面量构造
let p = Point { x: 1, y: 2 };
```

### 3.6 模块系统

```zeta
module math {
    pub const PI: f64 = 3.14159;
    pub fn square(x: i64) -> i64 { x * x }
}
import math::PI;                 // 别名导入
import math::square as sq;

// 多文件模块：module foo; → foo.zeta / foo/module.zeta（扁平名字空间，路径即「模块名::」）
// 跨模块路径：模块名::Enum::Variant / 模块名::CONST
```

### 3.7 数组与索引访问

```zeta
let arr = [10, 20, 30];        // 数组字面量（元素类型统一）
let arr2: [i64; 4] = [1, 2, 3, 4];  // 类型注解保留长度
let x = arr[0];                // 索引读取（越界编译期可查）
arr[1] = 99;                   // 索引写入（别名共享，互相可见）
let ch = s[0];                 // 字符串按字符索引（步长 1 字节）

// 动态切片（数组 / Vec / String 通用，元素按值拷贝返回全新缓冲）
let a = arr[1..<3];            // 半开 [1,3)：[20, 30]
let b = arr[lo...hi];          // 双闭；c = arr[lo<..hi] 不含下界
let c = v[lo..<hi];            // Vec 切片（std Vec::slice 泛型方法）
// 越界自动 clamp 到 [0, len]，start >= end 返回空
```

### 3.8 函数一等值（函数指针）

```zeta
fn add(a: i64, b: i64) -> i64 { a + b }     // 尾部表达式返回
fn apply(f: fn(i64, i64) -> i64, x: i64, y: i64) -> i64 { f(x, y) }

let f = add;                    // 函数值绑定（类型推断为 fn(i64, i64) -> i64）
let r1 = f(3, 4);               // 通过函数值间接调用：7
let r2 = apply(add, 10, 20);    // 函数值作实参传递：30
let g: fn(i64, i64) -> i64 = add;  // 显式类型注解

// H2 无捕获闭包：`|a, b| expr` desugar 为匿名函数 + 函数指针（零运行时开销）
let r3 = apply(|a, b| a + b, 10, 20);    // 30：闭包作 fn 形参实参
let inc: fn(i64) -> i64 = |x| x + 1;     // fn 注解绑定闭包
let r4 = inc(41);                        // 42：经函数值间接调用
```

// H3 捕获闭包（IIFE）：`(|x| body)(args)` 立即调用，body 引用外层变量按值捕获
let factor = 3;
let r5 = (|x| x * factor)(14);                 // 42：捕获 factor，desugar 为 __closure_N(factor, x)
let name = String::from("zeta");
let r6 = (|s| s + name)(String::from("hi "));  // hi zeta：捕获 name

> H2 无捕获闭包约束：参数无类型注解，需 fn 类型上下文驱动推断（fn 形参实参 / `let f: fn(..) = |..| ..` 注解）；闭包体仅可引用参数与字面量；参数模式仅支持简单标识符与 `_`；返回闭包的函数已支持（H5 补全：`fn make() -> fn(..) { |x| .. }` 尾闭包按 H2 签名检查生成函数指针返回）。
> H3 捕获闭包（IIFE MVP）：`(|x| body)(args)` 立即调用时 body 中引用的外层变量按值捕获，desugar 为匿名函数 `__closure_N(cap..., x)`（捕获变量作前置参数、参数名保留原名）+ 普通函数调用，零新增 IR 节点；字符串字面量实参自动升级为 String 语义；`move` 关键字 MVP 忽略（所有权宽松）。
> H5 闭包值对象（MVP）：`let f = |x: i64| body;` 绑定后 `f(..)` 反复调用——按值捕获 desugar 为捕获聚合对象（每捕获一槽 `Alloc` + `FieldSet`）+ 调用点 `__closure_N(FieldGet(f, i)..., 实参...)` 展开，零新增 IR 节点；参数类型确定规则：有注解用注解类型，无注解由首次调用点实参推断（延迟固化，`let f = |x| x + 1; f(41);`，半注解亦可用），从未调用则不检查闭包体（惰性）。H5 补全：**返回闭包的函数**（`fn make() -> fn(i64) -> i64 { |x| x + 1 }` 尾闭包按 H2 签名检查；`let f = |x: i64| x + 1; f` 作返回值时无捕获闭包值降级为 fn 指针；`let f = |x| x * 2; f` 未固化闭包按返回签名固化）；**闭包值作 fn 实参**（无捕获闭包值 → fn 形参降级/固化为函数指针 `apply(f, 41)`；有捕获闭包值经 fn 签名传递报 Unsupported/类型不匹配）。限制：仅按值捕获；捕获闭包值不跨函数边界（不能作 fn 实参/返回值）；`move` 忽略、按引用捕获规划中。

```zeta
// H4 trait 对象（dyn Trait）：&T 强制转换 + vtable 间接分派（2 槽胖指针）
trait Shape {
    fn area(&self) -> f64;
    fn sides(&self) -> i64;
}
struct Circle { radius: f64 }
impl Shape for Circle {
    fn area(&self) -> f64 { 3.14 * self.radius * self.radius }
    fn sides(&self) -> i64 { 0 }
}
struct Rect { w: f64, h: f64 }
impl Shape for Rect {
    fn area(&self) -> f64 { self.w * self.h }
    fn sides(&self) -> i64 { 4 }
}
let c = Circle { radius: 2.0 };
let r = Rect { w: 3.0, h: 4.0 };
let d1: dyn Shape = &c;          // &T → dyn Trait 强制转换
let d2: dyn Shape = &r;
println(d1.area());              // 12.56：vtable 分派到 Circle::area
println(d2.area());              // 12.0：分派到 Rect::area（多态）
let d3 = d1;                     // 胖指针拷贝共享同一 vtable
println(d3.sides());             // 0
```

> H4 `dyn Trait` 约束：trait 与 impl 均须非泛型；方法签名含 `Self`（关联返回类型 / 参数）不支持经 dyn 调用；vtable 的 drop/size/align 槽 MVP 置 0（显式释放语义与 `Box`/`Rc` 一致）。

---

### 3.9 MVP 已知限制（规划中特性）

以下语法可解析但 MVP **未实现**（typecheck 显式报 Unsupported），详见 [`docs/guide.md`](docs/guide.md) §13：

| 特性 | 说明 |
|------|------|
| 宏调用 | ✅ 已实现：`macro_rules!` 声明式宏（`$x:expr`/`$x:ident`/`$x:ty`/`$x:tt` 元变量 + `$(`...`)` 重复 `*`/`+`/`?`，parse 期递归展开为 AST）——`$x:expr` 捕获**完整表达式 token 序列**（token 级优先级爬升：前缀一元 `-`/`!`/`not`/`&`/`*`、二元中缀、后缀调用/索引/成员/`?`/`as` 转换、嵌套宏调用 `m!(x)`，按原样展开优先级由调用方负责）+ 内置格式化宏 `println!`/`print!`/`format!`/`dbg!` + `eprintln!`/`eprint!`（stderr，N4 ✅；`{}` 值占位、`{:?}` 同构、`{{`/`}}` 转义、多参数可变长度；typecheck desugar 为 String 拼接 + 内建打印，`eprint*` 经 POSIX `dprintf(2, ...)` 直写 stderr）+ 集合宏 `arr!`/`vec!`/`map!`（I3 ✅，§3.9：`arr!` → 数组字面量；`vec!`/`map!` → 块表达式 `Vec::with_capacity(n)` + 逐元素 `push`/`insert`，空集合 → `new()`，元素经子 Parser 解析支持嵌套宏调用与完整表达式）；`r#"..."#` 带哈希原始字符串已实现（lexer：任意 `#` 定界、无转义，与 `r#ident` 区分）；`!` 保留 `not` 一元运算符语义。限制：无卫生宏（hygiene） |
| 引用类型 | `&x`/`&mut x` 表达式、`&T`/`&mut T` 参数与返回、解引用 `*` 已实现（G1 ✅，标量存 `i8*` 槽、聚合拷贝指针、字段/方法自动剥引用层）；裸指针（G3 ✅）：`*const T`/`*mut T` 类型 + `*p` 读写 + `&T`↔`*const T` 互视（宽松）+ `*mut` 降级 `*const`；生命周期标注（G4 ✅ MVP 语法接受）：`<'a>` 与 `&'a T` 解析后丢弃（宽松检查）；`&str` 只读借用视图（G2 ✅：`String::as_str()` + `&str` 参数/返回/索引 + `String::from(&str)` 深拷贝）；**`str` 值一等类型**（字符串字面量 / 绑定字面量的变量：方法调用 / `+` 拼接 / 内容比较自动升级为 String 对象，编译期长度展开）；**String 形参位置的字面量实参自动升级**（`m.push_str("!")`/`m.contains("z")`/`map.insert("k", 1)`/`f("hi")` 直接可用——覆盖实例 / 静态方法、普通函数、函数指针、泛型调用与 `dyn Trait` 方法，与 IIFE / 闭包值实参升级语义一致）；**`ref` / `ref mut` 模式已实现**（G1 收尾：`match` 臂与 `let ref x = e;` 绑定变量为对匹配值的引用而非值拷贝，枚举子模式 / struct 字段 / 解引用写均可用；MVP 注意——`match` 先拷贝匹配值，`ref` 绑定指向拷贝，`ref mut` 写与原变量无关）；**严格借用检查已实现**（G1 收尾，Rust E0502/E0499/E0596/E0597 对应）：NLL 近似的借用排他性——`&mut` 与任何活跃借用互斥、多个 `&mut` 互斥、活跃可变借用期间写入被借用变量报 `BorrowConflict`；`&mut` 要求 `let mut` 绑定（`BorrowMutImmutable`）；局部引用逃逸函数（尾表达式 / `return` 返回 `&x` 或绑定引用变量）报 `DanglingReference`（参数来源引用允许返回）；语义有意宽松——读取被借用变量与经 `*p` 写入允许（裸指针别名合法），共享借用（多个 `&`）可共存，仅直接赋值被借用变量触发冲突；`print`/`println` 参数为引用时自动剥层打印解引用值（`println(r)` ≡ `println(*r)`）；生命周期标注（`'a`）仍为语法接受宽松检查，严格生命周期验证规划中 |
| 闭包 | ✅ H2 无捕获闭包已实现（§3.8）：`\|x, y\| expr` desugar 为匿名函数（`__closure_N`）+ 函数指针（typecheck `check_closure_expected` 按预期 fn 签名检查闭包体，注入全局 HirItem，零运行时开销）；需 fn 类型上下文（fn 形参实参 / fn 注解绑定）。✅ H3 捕获闭包（IIFE MVP）：`(\|x\| body)(args)` 立即调用按值捕获（迭代收集捕获变量 + 类型快照，desugar 为匿名函数 + 捕获变量前置调用，零新增 IR 节点）。✅ H5 闭包值对象（MVP，含补全）：`let f = \|x: i64\| ..; f(..)` 绑定后反复调用（按值捕获 desugar 为捕获聚合对象 + 调用点字段读取展开，零新增 IR 节点，仅局部变量环境）；参数类型规则——有注解用注解、无注解由首次调用点实参推断（延迟固化，半注解亦可用）；**返回闭包的函数已实现**（`fn make() -> fn(..) { \|x\| .. }` 尾闭包按 H2 签名检查；无捕获/未固化闭包值作返回值经 fn 指针降级/签名固化）；**无捕获闭包值可作 fn 实参**（降级/固化为函数指针）；限制：捕获闭包值不跨函数边界（作 fn 实参/返回值报错）、`move` 忽略、按引用捕获规划中 |
| 函数指针 | ✅ 已实现（H1）：`fn(T) -> R` 类型 + `let f = add` 函数值绑定 + `f(args)` 间接调用（typecheck `Type::Fn` → HIR/MIR/LIR `CallIndirect` → LLVM `i8*` 槽 + 按签名 `bitcast` + 间接 `call`）；函数值可作实参、返回值、重新绑定、类型注解；`&T` 已实现见上 |
| 运算符 | `?` 错误传播已实现（K1 ✅）：`expr?` 在 Option/Result 上下文 desugar 为 `match { Some(__v) => __v, None => return Option::None }`（Result：`Err(__e) => return Result::Err(__e)`），复用 check_match 的 if-else 链 + tag 比较，零新增 HIR 节点；支持表达式中间嵌套 `?`（如 `Some(a? + b?)`）；裸无参变体值表达式（`return None;`）可用；`?` 用于非 Option/Result 类型报 Unsupported。`dyn Trait` ✅ 已实现（H4，§3.8）：trait 对象（`dyn Trait` 类型 + `&T` 强制转换 → vtable + 2 槽胖指针 + 方法调用 vtable 间接分派），MVP 限制：非泛型 trait/impl、含 `Self` 签名方法不可经 dyn 调用 |
| 所有权层级 | K2 `Box<T>` + K3 `Rc<T>`/`Arc<T>` + K4 `Gc<T>` ✅（§3.11）：堆分配 + `*` 解引用 + 字段/方法/索引自动剥层 + 引用计数（clone/强弱计数/弱引用/`try_unwrap`）+ 可选 GC（`gc_region` 块 + 逃逸 root + 保守标记-清除，`zeta-gc-runtime`） |
| 并发 | `fmt` 模块规划；actor 的 `async` 方法 + `.await`/`send` 已实现（§3.3）；**actor 交叉编译 / WASM 支持（L4 ✅，guide.md §11.3：`wasm32-wasip1` 目标下 driver 注入静态 `zeta_actor_resolve` 符号表替代 dlsym + WASI 单线程同步运行时，ask/send/FIFO/受监督崩溃重启协议与 native 一致；需先 `cargo build --target wasm32-wasip1 -p zeta-actor-runtime`）**；普通函数 `async fn`/`.await` 已支持（**S1c ✅，guide.md §9.3 + std-lib.md §10.3：`async fn` desugar 为 Future 结构体 + poll 状态机 + 构造器（`zeta-desugar`），`expr.await` 经状态机轮询子 future，支持 `Poll::Pending` 挂起/恢复与跨 await 变量提升，`block_on` 轮询驱动；MVP 限制：参数 `i64`、返回 `i64`/`()`、await 仅限直线代码，控制流块内/表达式嵌套 await 规划中**）；`json` 序列化已实现（L2 ✅，§3.12：`json::stringify`/`json::parse::<T>`，turbofish 泛型实参；标量/数组/struct/Vec/`HashMap` 序列化 + `i64`/`bool`/`String`/`HashMap` 反序列化；`Serialize`/`Deserialize` trait 与 `#[derive]` 宏规划） |
| 迭代器 | ✅ J1–J3 已实现（§3.10）：`for x in arr` 数组迭代（索引遍历，长度编译期已知）+ 自定义迭代器接入 `for`（存在 `next() -> Option<T>` 方法，inherent/trait impl，desugar 为 `loop { match it.next() { Some(x) => body, None => break } }`）+ 适配器 `map`/`filter`/`fold`/`collect`/`take`/`skip`（数组/Vec/迭代器接收者，经 H2 闭包，返回 `Vec<T>` 可链式）；数值区间、`for x in vec`/`for (k, v) in map` 亦可用；`Iterator` trait 定义（std-lib §2.3）为规划 API（适配器为编译器内建） |
| region 选项 | ✅ `adaptive`/`with_size (N)`/`strategy (bump)` 已实现（L3，§3.4：region 指令接线 `zeta-region-alloc` C ABI 运行时 + `--profile` PGO 画像回灌 `adaptive` 初始容量）；`strategy (pool)` 等规划中 |
| `String::from(s)` | ✅ 支持字面量（及绑定字面量的变量）、运行期 `String` 变量（`≡ s.clone()` 深拷贝）、`&str` 视图（读 data/len 槽深拷贝）；G2 已消除长度表达限制 |

---

### 3.9a 集合宏（I3 ✅）

```zeta
// arr! → 数组字面量（元素类型须统一，长度编译期已知）
let a = arr![1, 2, 3];
println(a[0] + a[1] + a[2]);          // 6

// vec! → Vec<T>（desugar：with_capacity(n) + 逐元素 push，空 → Vec::new()）
let v = vec![10, 20, 30];
println(v.len());                     // 3
let e = vec![];
println(e.len());                     // 0

// map! → HashMap<K, V>（desugar：with_capacity(n) + 逐元素 insert，空 → new()）
let m = map![1 => 10, 2 => 20];
println(m.len());                     // 2
match m.get(2) {
    Option::Some(x) => println(x),    // 20
    Option::None => println(0),
}
let me = map![];
println(me.len());                    // 0

// 元素为任意表达式（含嵌套宏调用、绑定变量）
let base = 5;
let vb = vec![base, base + 1, base * 2];  // [5, 6, 10]

// 结果可继续可变操作
let mut mv = vec![1, 2];
mv.push(3);
println(mv.len());                    // 3
```

> I3 集合宏约束：`arr!` parse 期直接产出 `ArrayLit`；`vec!`/`map!` desugar 为块表达式（`let mut __vec_N`/`__map_N` + `.push(e)`/`.insert(k, v)` + 块值 = 集合变量），零新增 IR 节点；元素经子 Parser（继承宏注册表）解析，支持嵌套宏调用与完整表达式；`map!` 元素须为 `k => v`（缺 `=>` 报 parse 错）；空集合 → `Vec::new()`/`HashMap::new()`；`Vec::with_capacity`/`HashMap::with_capacity` 为 typecheck 构造器特判，`push`/`insert` 为 std 方法调用（详见 memory-model.md 与 std-lib.md）。

---

### 3.10 迭代器与适配器（J1–J3）

```zeta
// J1 数组迭代：`for x in arr`（索引遍历，长度编译期已知）
let arr: [i64; 4] = [1, 2, 3, 4];
let mut sum = 0;
for x in arr {
    sum += x;
}  // sum = 10

// J2 自定义迭代器接入 for：存在 `next() -> Option<T>` 方法的类型
struct Counter { limit: i64, pos: i64 }
impl Counter {
    fn new(limit: i64) -> Counter { Counter { limit: limit, pos: 0 } }
    fn next(&mut self) -> Option<i64> {   // inherent 或 trait impl 均可
        if self.pos >= self.limit { return None; }
        let v = self.pos;
        self.pos += 1;
        Some(v)
    }
}
let mut s = 0;
for v in Counter::new(5) { s += v; }  // 0+1+2+3+4 = 10

// J3 适配器（数组 / Vec / 迭代器接收者，经 H2 无捕获闭包，返回 Vec<T> 可链式）
let d = [1, 2, 3, 4].map(|x| x * 2);          // Vec: [2, 4, 6, 8]
let e = [1, 2, 3, 4, 5].filter(|x| x % 2 == 1); // Vec: [1, 3, 5]
let t = [1, 2, 3, 4].fold(0, |acc, x| acc + x); // 10
let c = [7, 8, 9].collect();                  // Vec: [7, 8, 9]
let tk = [1, 2, 3, 4, 5].take(3);             // Vec: [1, 2, 3]
let sk = [1, 2, 3, 4, 5].skip(2);             // Vec: [3, 4, 5]
let chained = Counter::new(6).filter(|x| x > 1).map(|x| x * x); // Vec: [4, 9, 16, 25]
```

MVP 约束：适配器参数必须是 H2 无捕获闭包（`|x| ..`，捕获外部变量报错）；适配器返回 `Vec<T>`（急切求值，非惰性迭代器），结果 Vec 可作下一适配器源；`Iterator` trait 定义（std-lib §2.3）为规划 API（适配器为编译器内建 desugar）。

---

### 3.11 堆分配 / 引用计数 / 可选 GC `Box<T>` / `Rc<T>` / `Arc<T>` / `Gc<T>`（K2–K4）

```zeta
// K2 Box::new 堆分配 + * 解引用（标量 load / 聚合指针拷贝，与 &T 同构）
let b = Box::new(42);
println(*b);                // 42

// 显式类型注解 / 嵌套装箱
let bi: Box<i64> = Box::new(7);
let bb = Box::new(Box::new(10));
println(**bb);              // 10

// 聚合 T：字段 / 方法 / 索引自动剥 Box 层
struct Point { x: i64, y: i64 }
let bp = Box::new(Point { x: 10, y: 20 });
println(bp.x + bp.y);       // 30（字段剥层）
let p = *bp;                // 聚合解引用（指针拷贝）
println(p.x);               // 10

let bs = Box::new(String::from("hello"));
println(bs.len());          // 5（方法剥层）
println(bs[0]);             // 104（索引剥层）

let mut bv: Box<Vec<i64>> = Box::new(Vec::with_capacity(2));
bv.push(5);
println(bv.len());          // 2（&mut self 方法经 Box）

let b2 = b;                 // Box 赋值 = 指针共享（浅拷贝，MVP 语义）

// K3 Rc<T> 引用计数：clone 共享同一 RcInner，强/弱计数可查
let r = Rc::new(42);
let r2 = r.clone();
println(r.strong_count());  // 2
println(*r2);               // 42

// 弱引用：downgrade → Weak<T>，upgrade 强计数 > 0 返回 Some(Rc)
let w = r.downgrade();
println(r.weak_count());    // 1
match w.upgrade() {
    Option::Some(rc) => println(*rc),   // 42
    Option::None => println(0),
}

// try_unwrap：强计数 == 1 → Ok(T)，否则 Err(Rc<T>)
match r.clone().try_unwrap() {
    Result::Ok(x) => println(x),
    Result::Err(rc) => println(*rc),    // 42（Err 分支）
}

// Arc 与 Rc 同构（计数槽原子性规划中）
let a = Arc::new(7);
let a2 = a.clone();
println(a.strong_count());  // 2

// K4 Gc<T> 追踪 GC（MVP 保守标记-清除，zeta-gc-runtime）：
// gc_region 块内分配，块结束触发 GC 周期（未逃逸对象回收）
gc_region {
    let a = Gc::new(42);
    println(*a);            // 42
}

// 逃逸对象：块返回值存活到块外（zeta_gc_escape 登记为 root）
let g = gc_region { let inner = Gc::new(100); inner };
println(*g);                // 100

// 字段 / 方法 / 索引自动剥层（与 Box / Rc 同构）
let gp = gc_region { Gc::new(Point { x: 5, y: 6 }) };
println(gp.x + gp.y);       // 11

// 嵌套 gc_region：内层逃逸对象在外层块内仍活跃（存活链式提升）
let outer = gc_region {
    let o = Gc::new(1000);
    let saved = gc_region { Gc::new(2000) };
    o
};
println(*saved);            // 2000
println(*outer);            // 1000

// 块外分配（epoch 0，永不回收——MVP 泄漏语义，仍可用）
let leaky = Gc::new(999);
gc_region { println(*leaky + *Gc::new(1)); }  // 1000
```

MVP 约束：`Box<T>`/`Rc<T>`/`Arc<T>`/`Weak<T>`/`Gc<T>` 为编译器内建（无 std 结构体定义，typecheck 特判）；`Box<T>` 布局 = 栈上 1 指针槽 + 堆上 `slot_count(T)` 个 8 字节槽；`Rc<T>` 布局 = 堆 `RcInner` 的 `T` 值区自堆首槽起（与 Box 同构）+ 尾部两计数槽（strong = 值区槽数、weak = +1），`Rc<T>` 栈上 1 槽指向 RcInner（详见 memory-model.md §4 MVP 注记）；`Gc<T>` 栈上 1 槽指向堆 1-槽包装（槽 0 存 `GcInner` 基址），对象与 `Box` 同构（详见 memory-model.md §5 MVP 注记）；聚合 T 装箱整槽区 memcpy（浅拷贝）；无自动 drop（计数只增不减，显式释放语义规划，与 `Vec`/`String` 一致）；`gc_region` 块 desugar 为 `zeta_gc_region_begin`/`zeta_gc_alloc`/`zeta_gc_escape`/`zeta_gc_collect`，块外对象永不回收、跨块逃逸对象引用图泄漏至程序结束，多线程 / 增量回收 / write barrier 规划中。

### 3.12 JSON 序列化 / 反序列化（L2 ✅）

`json::stringify(v)` 序列化 / `json::parse::<T>(s)` 反序列化——**编译器内建**，typecheck 期 desugar 为 String 构建/解析表达式，**零新增 IR 节点**。

```zeta
fn main() {
    // stringify：标量 / String / &str（转义）/ 数组 / struct / Vec / HashMap
    println(json::stringify(42));                          // 42
    println(json::stringify(true));                        // true
    println(json::stringify("a\"b"));                      // "a\"b"
    println(json::stringify([1, 2, 3]));                   // [1,2,3]

    struct Point { x: i64, y: i64 }
    let p = Point { x: 10, y: 20 };
    println(json::stringify(p));                           // {"x":10,"y":20}

    let v: Vec<i64> = vec![1, 2, 3];                       // 元素类型须确定（注解）
    println(json::stringify(v));                           // [1,2,3]

    let m: HashMap<i64, i64> = map![3 => 30, 1 => 10, 2 => 20];
    println(json::stringify(m));                           // {"1":10,"2":20,"3":30}（键序确定性）

    // parse：turbofish 泛型实参指定目标类型，直接返回 T
    println(json::parse::<i64>("42"));                     // 42
    println(json::parse::<bool>("true"));                  // true
    let s = json::parse::<String>("\"a\\\"b\"");
    println(s.len());                                      // 3（还原为 "a"b）

    // parse HashMap（`{"k":v,...}`；空 {} → 空 map）
    let m2 = json::parse::<HashMap<i64, i64>>("{\"1\":10,\"2\":20}");
    println(m2.len());                                     // 2
    match m2.get(1) {
        Option::Some(x) => println(x),                     // 10
        Option::None => println(0),
    }
}
```

- **turbofish**：`json::parse::<T>(s)` 类型实参 `::<T>`（parser `check_turbofish` 三 token 前瞻，`ExprKind::Call.type_args` 承载，AST 层零新增节点；嵌套泛型 `>>` 按 pending 深度拆分，`::<HashMap<i64, i64>>(s)` 可用）。
- **stringify 支持**：`i64`→十进制、`bool`→`true`/`false`、`String`/`&str`→带引号 JSON 字符串（`"` `\` 换行 制表 → `\"` `\\` `\n` `\t`）、数组→静态展开、struct→`{"f":v,...}`（字段序=定义序、嵌套递归）、`Vec<T>`→while 循环（元素类型须确定——`vec![...]` 绑定后为 `Vec<Infer>`，须 `let v: Vec<i64>` 注解，与 `for x in v` 约束一致）、`HashMap<K,V>`→`{"k":v,...}`（L2f：i64/String 键 + 值递归，键序确定性——`for (k, v) in m` 容量扫描顺序）。
- **parse 支持**：`i64`/`bool`/`String`/`HashMap<K,V>`（L2g：i64/String 键 + 标量值 i64/bool/String，`substring`/`split(",")`/`find(":")` 分段 + 键 `json_unescape` + 值递归解析 + `HashMap::new()`/`insert` 构建；空 `{}` → 空 map）；MVP 语义直接返回 `T`（非法输入给默认值 `0`/`false`/空串/空 map），非 Result 包装。
- 转义函数 `json_escape`/`json_unescape` 实现于 std `core.zeta`；LLVM 字符串常量 `"` 用十六进制 `\22`（`\"` 被 clang 误解析为 `[1 x i8]` 长度不匹配）。
- MVP 限制：`map![...]` 绑定后 K/V 为 `Infer`，须 `let m: HashMap<i64, i64>` 注解（与 `vec![...]` 一致）；HashMap parse 键/值含逗号或冒号时 `split(",")`/`find(":")` 分段不可靠、嵌套 `HashMap` 值报 Unsupported（值限标量）；自定义 `Serialize`/`Deserialize` trait 与 `#[derive]` 宏规划中（std-lib §9）。

---

## 4. 编译器架构

### 4.1 编译流水线

```
源代码 (.zeta)
    │
    ▼
┌─────────────────────────────────────┐
│  Lexer (zeta-lexer)                │  → Token 流
├─────────────────────────────────────┤
│  Parser (zeta-parser)              │  → AST
├─────────────────────────────────────┤
│  Name Resolution & Macro Expand     │  → HIR
├─────────────────────────────────────┤
│  Type Checker (zeta-typecheck)      │  → 类型标注的 HIR
├─────────────────────────────────────┤
│  Borrow Checker (zeta-borrowck)     │  → 借用验证
├─────────────────────────────────────┤
│  Region Checker (zeta-regionck)     │  → 区域验证 + 大小推断
├─────────────────────────────────────┤
│  MIR Lowering (zeta-mir)           │  → 控制流图
├─────────────────────────────────────┤
│  Optimization Passes                │  → 常量折叠、内联、死代码消除
├─────────────────────────────────────┤
│  LIR Lowering (zeta-lir)           │  → 目标无关低级 IR
├─────────────────────────────────────┤
│  Code Generation (zeta-codegen)     │  → LLVM IR / Cranelift IR
├─────────────────────────────────────┤
│  Backend (LLVM / Cranelift)         │  → 机器码 / WASM
└─────────────────────────────────────┘
    │
    ▼
可执行文件 / .wasm
```

### 4.2 增量编译策略

| 策略 | 粒度 | 触发条件 |
|------|------|----------|
| 模块级缓存 | 每个 .zeta 文件 | 接口哈希未变 |
| 函数级并行 | 无依赖函数 | 编译图分析 |
| 类型检查缓存 | 类型推导结果 | 输入类型未变 |
| PGO 数据复用 | 区域大小预测 | .zeta_profile 存在 |

### 4.3 目标性能

| 指标 | 目标 |
|------|------|
| 百万行全量编译 | < 30 秒 |
| 增量编译（单文件修改） | < 3 秒 |
| 编译器内存峰值 | < 2GB |
| 生成的代码性能 | 与 Rust 同级（±5%） |

---

## 5. 工具链命令

| 命令 | 功能 | 状态 |
|------|------|------|
| `zeta new <name>` | 创建新项目（`--lib` 库项目；生成 Zeta.toml + src/main.zeta 或 lib.zeta） | ✅ 可用（委托 zep 脚手架） |
| `zeta build` | 编译项目 | ✅ 可用（MVP；`--target <triple>` 交叉编译，macOS 双架构已验证） |
| `zeta run` | 编译并运行 | ✅ 可用（MVP） |
| `zeta test` | 运行测试 | ✅ 可用（D1：`tests/` 目录 compile-pass/compile-fail/run-pass） |
| `zeta fmt` | 代码格式化 | ✅ 可用（D2：AST 重建，`--check`/`-w`/`--indent`） |
| `zeta check` | 静态分析 | ✅ 可用（D2：未使用变量/恒常条件/冗余比较/不可达代码） |
| `zeta bench` | 基准测试 | ✅ 可用（D3：编译 + 多次计时统计，`--runs`/`--warmup`） |
| `zeta doc` | 生成文档 | ✅ 可用（D3：`///` 注释提取 → Markdown，`--out`/`--title`） |
| `zeta publish` | 发布包 | ✅ 可用（E3：委托 zep 打包发布，`--registry`/`--verbose`；重复版本拦截） |
| `zeta lsp` | 语言服务器 | ✅ 可用（F1：LSP over stdio，full 文档同步 + 诊断推送，复用 zeta-check 静态分析） |
| `zeta profile` | PGO 回灌报告 | ✅ 可用（F2：`.zeta_profile` → 区域大小预测报告；`zeta build --profile` 编译期注入） |

---

## 5.5 编译器加固与标准库扩展（新版开发计划执行记录，2026-08）

> 本节记录新版开发计划（阶段 A–F，详见 [`docs/development-plan.md`](docs/development-plan.md)）的执行进度：
> **阶段 A 全部完成（A1–A4 ✅），阶段 B 全部完成（B1–B5 ✅），阶段 C 全部完成（C1–C3 ✅），阶段 D 全部完成（D1–D3 ✅：zeta test / zeta fmt / zeta check / zeta doc / zeta bench），阶段 E 全部完成（E1 交叉编译 `--target` macOS 双架构 + `__zeta_target_os` 平台内建消除 `sockaddr_in4` 布局假设；E2 WASM 目标：`--target wasm32-wasi` 编译 + wasmtime 运行验证；E3 发布流程：zep publish 重复版本保护 + `zeta publish` CLI + release.yml 四平台 + CHANGELOG.md），阶段 F 全部完成（F1 LSP 服务器 MVP：`zeta lsp` + 新 crate `zeta-lsp`，文档同步 + 诊断推送；F2 PGO 数据回灌：`zeta profile` 命令 + `zeta build --profile` 编译期注入，`.zeta_profile` → 区域大小预测报告）**，E1 Windows/ARM 工具链（待对应环境）待做。
> **后续计划（编译器能力 G–L 已完成；标准库深度完善 M–T 进行中）**：见 [`docs/mvp-gaps-plan.md`](docs/mvp-gaps-plan.md)（§2/§3 阶段 G–L 编译器能力补齐 + §3b 阶段 M–T 标准库深度完善——错误处理基底 M ✅ → 文件系统 N ✅ → 网络 O → 并发通道 P → 序列化/格式化 trait Q → 高性能 IO R → 异步运行时 S（前置：线程支持 S0）→ 集合收尾 T）。

- [x] **N 阶段：文件系统与 IO 对象化 + `eprintln!`/`eprint!` 宏**（§3.9 宏调用行更新；mvp-gaps-plan.md §3b/§4；guide.md §13；std-lib.md §4.2/§8）：**N1 File 对象化**——`File::open`/`create` + `read_to_string`/`read`/`write`/`write_all`/`flush`/`metadata`/`size`（io.zeta 直接 libc extern + `open_mode_str` 模式映射）。**N2 stdin/stdout/stderr 模块**——`stderr()`/`stdout()` `write`/`writeln`/`flush` + stdin `read_to_string`/`lines`。**N3 `Path`/`fs`**——`Path::new`/`join`/`parent`/`file_name`/`extension`/`exists`/`is_file`/`is_dir` + `fs::read_to_string`/`write`/`copy`/`remove_file`/`remove_dir_all`/`rename`/`create_dir`/`create_dir_all`/`read_dir`。**N3c 关键修复（`r#rename` 遮蔽）**：`fs.zeta` 内裸名 `rename(...)` 被 `resolve_callable` 模块内优先规则绑定到 `fs::rename`（返回 `Result<i64, IoError>`），`if r != 0` 中 Result 与整数比较触发 ChainTypeMismatch——为 `r#` 前缀承载「根命名空间显式引用」语义：lexer `read_raw_ident` 保留前缀产出 `Ident("r#rename")`；parser `parse_fn` 声明名归一化；typecheck `resolve_callable` 对 `r#xxx` 去前缀并跳过模块内优先直接绑定根命名空间；`register_use` 归一化 use 路径段。**N4 `eprintln!`/`eprint!`**：parser `is_builtin_macro` + typecheck `builtin_signature`（0–1 参数）/`check_call` String 分支映射 `eprint_string`/`eprintln_string`/宏分发（`trim_end_matches('!')` 通用）+ LIR/codegen `BUILTIN_FUNCTIONS` 注册 4 内建；codegen 打印分支统一 `emit_out` 辅助闭包——stderr 目标用 **POSIX `dprintf(2, fmt, ...)`**（不引用 `stderr` 符号：macOS libc 为 `__stderrp` 不可移植；WASI 亦提供 dprintf），非 String 参数复用 `builtin_fmt`（`eprintln(7)`/空参换行/Char/Bool 全支持）。产出 `tests/run-pass/{file_io,open_mode,stdout_stderr,stdin_enhance,path_ops,fs_ops,fs_dir,eprintln}.{zeta,out}` + `tests/compile-fail/io-result-misuse.zeta` + 全量 78 用例全绿 + cargo test 全绿 + clippy 0 警告。
- [x] **L4 平台加固：WASI net 禁用文档化 + Actor 交叉编译 / WASM 支持**（§3.9 并发行更新；mvp-gaps-plan.md §4；guide.md §11.3/§13 更新）：消除 guide.md §13 约束 2/3。**L4a net**：`net.zeta` 网络函数（`socketpair_stream`/`send_all`/`recv_some`/`tcp_connect`/`hostname`）加 `if __zeta_target_os() == 5 { return …; }` WASI 短路（码 5 = WASI，driver `target_os_code` 新增 wasi 分支，wasm 码 0→5 测试同步）。**L4b Actor WASM**（wasm32-wasip1，旧名 wasm32-wasi 废弃）：① 符号解析——dlsym 在 WASI 不可用，driver 从 HIR 收集 actor 符号（`escape_llvm_c_string`/`llvm_quoted_name`/`actor_symbols_from_ir`）注入静态表 `actor_resolve_ir`（`zeta_actor_resolve(name, nlen)` 逐条 `strcmp` → `ptrtoint` 函数地址），**外部链接**（internal 对静态库不可见）；② WASI 单线程同步运行时（`zeta-actor-runtime/src/sync.rs`，`#[cfg(target_os = "wasi")]`）：`Entry { handler, state: u64, factory: Option<ZetaFactory>, running }` + `TABLE: Mutex<Vec<Option<Entry>>>` + `NEXT: AtomicU64`（句柄 = 索引+1），`spawn`/`spawn_supervised`/`dispatch`（回调取快照防 Mutex 重入；`u64::MAX` = 崩溃 → factory 重建重启）/`stop`/`status`/`shutdown`；③ **ABI 修正**——`CallbackActor.state: u64`（原 `*mut c_void`）、`zeta_actor_spawn(handler: *const c_char, state: u64)`、`ZetaFactory = unsafe extern "C" fn() -> *mut c_void`（匹配 `__state_new` 的 `i8*` 返回，避免 wasm `signature_mismatch` trap）；④ driver `wasm_actor_runtime_lib_path` + `assemble_wasm` 探测链接（缺失且用了 actor 显式报错）。**关键教训**：clang 拒绝 `declare void @strdup(i8*)`（wasi-libc 无 strdup）→ `resolve_symbol` cfg dlsym/静态表双分支；`Vec::resize` E0599（Entry 非 Clone）→ `#[derive(Clone, Copy)]`。产出 `wasm_target_test.rs` 新增 `wasm_actor_runtime_ready`/`wasm_actor_runs`（ping-pong，输出 `11\n12\n2\n4\n`）/`wasm_actor_supervised_runs`（受监督崩溃重启，输出 `50\n0\n10\n`）+ 全量 cargo test 全绿。
- [x] **H4 `dyn Trait` trait 对象**（§3.9 运算符行更新；mvp-gaps-plan.md §4；guide.md §13 更新）：`dyn Trait` 类型 + `&T` 强制转换 + vtable 间接分派全链路（parser `ty.rs` `dyn` 分支 + AST `AstType::Dyn` + typecheck `Type::Dyn`）。**布局**：2 槽胖指针（槽 0 = 数据指针、槽 1 = vtable 指针）。**转换 `coerce_to_dyn`**：`let d: dyn Shape = &c;` desugar 为 HIR 块——`Alloc(3+N)` vtable（槽 0–2 = drop/size/align，MVP 置 0；槽 3.. = `FnPtr("Trait::method")` 按 trait 声明序）+ `Alloc(2)` 胖指针（数据指针 + vtable 指针）。**调用**：`check_method_call` Dyn 分支读 `FieldGet(obj,0)` 数据指针 + `FieldGet(obj,1)` vtable 指针 + `Index(__vtp, 3+idx)` 取方法函数指针 + `CallIndirect`——同一签名分派到不同 impl（多态）。MVP 限制：trait/impl 均须非泛型；方法签名含 `Self` 不可经 dyn 调用。产出 `tests/run-pass/dyn_trait.{zeta,out}`（6 输出）+ 全量 40 用例全绿 + cargo test 全绿。
- [x] **G3 裸指针 / G4 生命周期标注**（§3.9 引用类型行更新；mvp-gaps-plan.md §4；guide.md §13 更新）：**G3** `*const T`/`*mut T` 类型 + `*p` 读写（parser `Token::Star` 分支、AST `AstType::RawPtr`、typecheck `Type::RawPtr`、Deref 分支扩展）+ 引用↔裸指针互视宽松规则（`compatible_with` 双向——参数检查是实参×形参方向，与 let 声明相反，单方向规则会在函数调用处失效）+ `*mut` 降级 `*const`；codegen 与引用同为 `i8*` 槽零改动；zeta-fmt/zeta-doc `fmt_type` 补分支。**G4** 生命周期标注 MVP 语法接受：`parse_generics` 跳过 `'a`（含 `: 'b` bound）、`parse_type` 的 `&` 分支跳过 `&'a T`，typecheck 解析后丢弃（宽松检查）；borrowck 生命周期验证规划中。产出 `tests/run-pass/raw_ptr.{zeta,out}`（7 输出）+ `tests/run-pass/lifetime.{zeta,out}`（4 输出）+ 全量 38 用例全绿。
- [x] **H3 捕获闭包（IIFE MVP）**（§3.8 扩写 + §3.9 闭包行更新；mvp-gaps-plan.md §4；guide.md §13 更新）：`(|x, y| body)(args)` 立即调用时 body 引用的外层变量按值捕获，desugar 为匿名函数 `__closure_N(cap1, cap2, x, y)`（捕获变量作前置参数、参数名保留原名）+ 调用点普通函数调用，零新增 IR 节点。实现：`check_call` 的 `_ =>` 分支拦截 `ExprKind::Closure` callee → `check_capture_closure_iife`；**捕获收集迭代重查法**（环境 = 已发现捕获 + 闭包参数，逐轮把 UndefinedVariable 中存在于外层环境的名称收为捕获并重查，循环收敛）；字符串字面量实参经 `check_string_from` 升级 String 语义（与 `let s = "..."` 绑定一致，槽数匹配）。MVP 约束：仅 IIFE（闭包值对象 = 匿名结构体 + call 方法规划中）、`move` 忽略、嵌套捕获不支持、参数遮蔽捕获（Rust 语义）。产出 `tests/run-pass/capture_closure.{zeta,out}`（8 输出，含 H2 回归/参数遮蔽/for 循环内 IIFE）+ 全量 39 用例全绿 + cargo test 全绿。
- [x] **H5 闭包值对象补全（返回闭包的函数 / 非注解绑定 / fn 实参）**（§3.8 H5 约束扩写 + §3.9 闭包行更新；guide.md §3.2 H5 章节扩写 + §13 更新）：① **返回闭包的函数**——`fn make() -> fn(i64) -> i64 { |x| x + 1 }`：`check_block_with_expected_final` 把预期 final 类型传给尾表达式检查（fn 返回类型 → `check_closure_expected` 按 H2 无捕获签名检查生成函数指针）；`let f = |x: i64| x + 1; f` 作返回值 → `try_closure_value_as_fn`（无捕获闭包值降级为 fn 指针）；`let f = |x| x * 3; f` 未固化闭包作返回值 → `fix_deferred_closure_with_sig` 按返回签名固化参数类型。② **非注解/半注解闭包值绑定**——`let f = |x| x + 1; f(41);`：绑定处无 fn 上下文且参数无注解时注册**延迟闭包**（`ctx.deferred_closures`，`Type::Closure{ fn_name: "" }` 标记未固化，绑定 HIR = 占位 `Alloc{slots:0}`）；首次调用 `check_deferred_closure_call`：实参推断参数类型（有注解用注解并校验兼容——半注解 `|x: i64, y|` 亦可用；无注解由实参推断，String 字面量升级语义与 IIFE 一致）→ `check_closure_body_with_captures` 检查闭包体 + 收集捕获 → 生成匿名函数 → 捕获对象内联构造到调用点块尾并**重绑定变量**（后续调用读取重绑定的聚合对象）→ 变量完整闭包类型回写；从未调用则惰性不检查闭包体。③ **无捕获闭包值作 fn 实参**——`apply(f, 41)`：fn 形参签名上下文经 `try_closure_value_as_fn`/`fix_deferred_closure_with_sig` 降级/固化为函数指针；有捕获闭包值经 fn 签名传递报 Unsupported（未固化）/类型不匹配（已固化）。**关键教训**：占位 `Alloc{slots:0}` 需处理零槽分配（升级为 1 槽 Ptr 或空对象均可用）；延迟闭包绑定 HIR 槽匹配（body_ty 用完整闭包类型）；捕获变量 FieldSet 用 `field_scalar_of` 解引用避免基址漂移。产出 `tests/run-pass/closure_fn_return.{zeta,out}`（6 输出）+ `tests/run-pass/closure_value_half_anno.{zeta,out}`（5 输出）+ `tests/run-pass/closure_lazy.{zeta,out}`（惰性检查）+ `tests/compile-fail/closure-value-{anno-conflict,capture-fnarg,capture-return,capture-return-unfixed,body-undefined}.zeta`（5 个负面用例）+ 全量 66 用例全绿 + `cargo test --workspace -- --test-threads=1` 118 个 test target 全绿（driver 集成测试并行 spawn clang 进程资源耗尽 os error 35，CI 已同步改串行）。
- [x] **K4 `Gc<T>` 追踪 GC（MVP 保守标记-清除）**（§3.9 所有权层级行更新、§3.11 扩写；guide.md §8.4/§13 更新；memory-model.md §5 MVP 注记；std-lib.md §11 更新）：编译器内建（`check_gc_new`/`check_gc_region`）+ 独立运行时 crate `zeta-gc-runtime`。**布局**：`Gc<T>` 栈上 1 槽（Ptr）指向堆 1-槽包装（`Alloc{slots:1}`，槽 0 存 `GcInner` 基址）；对象 = `slot_count(T)` 个 8 字节槽连续堆块，与 `Box<T>` 完全同构（解引用/剥层零差异，`heap_ptr_hir` 统一 Box/Rc/Arc/Gc）。**生命周期协议**：`gc_region` 块 desugar 为 `zeta_gc_region_begin()`（`epoch += 1`）→ `zeta_gc_alloc(n)`（malloc + 注册块表 + 记录 epoch）→ `zeta_gc_escape(ptr)`（块返回值登记逃逸 root，空指针空操作）→ `zeta_gc_collect()`（从逃逸 root 标记 → 清除 `epoch` 匹配未标记对象 → 存活对象提升为 root → `epoch -= 1`）；块外对象 epoch 恒小 → 永不回收（MVP 泄漏语义）；跨块存活引用链经"存活提升"连续保护。**关键教训**：① `zeta_gc_escape` 参数必须传**对象 base**（`heap_ptr_hir` 解包装槽 0）——初版传 `Variable(esc)`（包装指针）致 `mark` 线性查找（`header.base == root.base`）失配、对象被误回收（悬垂读取）；② `collect` 存活提升条件须为**所有被标记对象**（`marked`）而非 `marked && epoch == s.epoch`——嵌套块 collect 释放全部旧 root 节点，仅提升本块对象会使外层逃逸对象（`epoch < s.epoch` 存活但未标记可达）失去保护、后续外层 collect 误回收；③ 调试必须 `--force`（增量缓存命中跳过 typecheck）。**运行时加固**：全部动态内存只用 `libc::malloc`/`free`（对象块 + header/root 链表），不用 Rust 堆分配与 `realloc`——macOS C 主程序环境实测 `RawVec`/`realloc` 在 `libc::malloc` 之后触发 `libsystem_malloc` `mfm_alloc` 崩溃（`_os_unfair_lock_unowned_abort` / SIGKILL）；全局状态 `SyncUnsafeCell` 单线程无锁（MVP 编译产物单线程串行调用约定，无 Mutex）。产出 `tests/run-pass/gc_region.{zeta,out}`（15 输出）+ `tests/compile-fail/gc_bad.zeta`（`Gc::new()` 参数个数断言）+ 全量 36 用例全绿 + cargo test 全绿。
- [x] **K3 `Rc<T>`/`Arc<T>` 引用计数装箱**（§3.9 所有权层级行更新、§3.11 扩写；guide.md §13 更新；memory-model.md §4 MVP 注记）：编译器内建（typecheck 特判，零新增 IR 节点）。布局：堆 `RcInner` 的 `T` 值区自堆首槽起（与 `Box<T>` 同构）+ 尾部计数槽（strong = `slot_count(T)`、weak = +1），`Rc<T>` 栈上 1 槽（Ptr）指向 RcInner。内建：`Rc::new`/`Arc::new`（值区写 + `FieldSet` 计数初始化 1/0）、`clone`（强计数 +1 指针共享）、`strong_count`/`weak_count`（返回 `Type::USize`）、`downgrade`（弱计数 +1 → `Weak<T>`）、`try_unwrap`（强计数 == 1 → `Ok(T)` / `Err(Rc<T>)`）、`Weak::upgrade`（强计数 > 0 → `Some(Rc<T>)` / `None`）。接线：`check_rc_method` 在 `heap_ptr_hir` 改写前分派（内建需原始 Rc 对象）；`heap_ptr_hir` 统一 Box/Rc/Arc（槽 0 即值区首槽）；`Rc::new`/`Arc::new`/`Weak::upgrade` 静态路径特判；Deref 分支扩展 Rc/Arc；`peel_refs_and_heap` 剥层。关键教训：`DerefSet` base 是地址、`FieldGet` 是 load 槽值，计数写必须用 `FieldSet`（GEP+store）；`try_unwrap` else 分支曾漏写 Err payload 槽致 match 读未初始化内存崩溃；`zeta run` 缓存 key 不含编译器版本，改编译器后须 `--force`。产出 `tests/run-pass/rc_new.{zeta,out}`（17 输出）+ `tests/compile-pass/rc_ty.zeta` + `tests/compile-fail/rc_bad.zeta` + 全量 33 用例全绿 + cargo test 全绿 + clippy 0 警告。
- [x] **K2 `Box<T>` 堆分配装箱**（§3.9 所有权层级行勾销、新增 §3.11；guide.md §13 更新）：`Box::new(v)` 编译器内建（typecheck 特判，无 std 结构体定义）+ `*` 解引用 + 字段/方法/索引自动剥层。布局：`Box<T>` 栈上 1 槽（Ptr）存堆指针，堆上分配 `slot_count(T)` 个 8 字节槽连续对象区（与对象槽区同构）。核心：`check_box_new`（`alloc_bytes(8*n)` + 标量 `DerefSet` 写堆首槽 / 聚合 `array_copy` 整槽区 memcpy 浅拷贝 + 1 槽 `Alloc` 返回）+ `type_slot_count`（槽数计算：标量 1 / struct 字段数 / enum `slot_count` / 数组长度 / 元组元素数 / 引用·Fn·内嵌 Box 1 槽）+ `peel_box`/`peel_refs_and_boxes`/`box_ptr_hir`（Box 表达式 → 堆对象指针 `FieldGet(box, 0, Ptr)`）。接线五处：Deref 分支加 Box（标量 load / 聚合指针拷贝，与 `&T` 同构）、字段访问剥层+base 改写、方法调用 receiver 改写（`Box<String>` len/索引、`Box<Vec<i64>>` push、`as_str` 特判）、索引 base 改写。`Vec::with_capacity` 无上下文返回 `Vec<Infer>`，测试经 `Box<Vec<i64>>` 注解统一。嵌套 `Box<Box<i64>>`（`**bb`）与 Box 赋值指针共享可用；无自动 drop（与 Vec/String 一致）。产出 `tests/run-pass/box_new.{zeta,out}`（15 输出）+ `tests/compile-pass/box_ty.zeta` + `tests/compile-fail/box_bad.zeta` + 全量 30 用例全绿 + cargo test 全绿 + clippy 0 警告。
- [x] **J1–J3 迭代器与集合协议**（§3.9 迭代器行勾销；guide.md §13 更新）：
  - **J1 数组迭代**（§3.10）：`for x in arr` desugar 为索引遍历循环（数组以指针存储、长度编译期已知，元素读取复用 `HirExpr::Index` 步长 8 / u8 按字节）；`check_for` 分派加 `Type::Array` 分支（`check_for_array`）。
  - **J2 自定义迭代器接入 for**（§3.10）：接收者类型存在 `next() -> Option<Item>` 方法（inherent / trait impl）→ `check_for_iterator` 构造 AST `let mut __for_it = it; loop { match __for_it.next() { Some(__elem) => { let pat = __elem; body }, None => break } }`（复用 check_method_call / check_match 全链路）；配套修复 parser `stmt_terminator` 补 `,`（`match { None => break, }` 臂体）。
  - **J3 适配器**（§3.10）：`map`/`filter`/`fold`/`collect`/`take`/`skip` 内建 desugar（`check_method_call` 特判，接收者为数组 / `Vec<T>` / 自定义迭代器）：新增 `closure_return_ty`（闭包实际返回类型预推断 → 收集容器类型注解，避免 `Vec<Infer>`）、`ty_to_ast`、`try_check_adapter`/`check_iterator_adapter`。闭包经 `check_closure_expected` 按 `fn(T...) -> U` 注入匿名函数（H2，返回函数指针）；收集容器 `let mut __out: Vec<U> = Vec::new();`；循环复用现有分派（数组/Vec 走 for，迭代器走 loop + next）；apply 语义 map/filter/fold/take/skip/collect（take 计数 break、skip 计数 push）；结果 Vec 可链式。
  - 产出 `tests/run-pass/array_for.{zeta,out}`（5 输出）+ `tests/run-pass/iterator_for.{zeta,out}`（6 输出）+ `tests/run-pass/adapters.{zeta,out}`（13 输出，含链式）+ `tests/compile-fail/adapter-badargs.zeta`（非闭包参数断言）+ 全量 27 用例全绿 + cargo test 全绿 + clippy 0 警告。
- [x] **K1 `?` 错误传播运算符**（§3.9 运算符行勾销；guide.md §13 更新）：`expr?` 在 Option/Result 上下文 desugar 为 `match` + `return` 早返回，零新增 HIR 节点（复用 check_match 的 if-else 链 + tag 比较）。typecheck：拆分 `check_match_with_scrutinee`（预推断 scrutinee，防内嵌闭包重复 desugar）+ `check_question`（infer inner → `Type::Named("Option"/"Result")` 特判 → 构造 AST MatchArm `Some(__v) => __v` / `None => return Option::None`、`Err(__e) => return Result::Err(__e)` 失败变体经完整路径构造 → `check_match_with_scrutinee`）；非 Option/Result 类型报 Unsupported。配套：parser 后缀循环 `?`（`ExprKind::Question`）；裸无参变体值表达式（`return None;`）经 infer_expr Ident 分支 `split_variant_path` 兜底；zeta-fmt（PREC_POSTFIX）/zeta-check（walk_expr）补分支。产出 `tests/run-pass/question.{zeta,out}`（5 输出，含表达式中间嵌套双 `?`）+ `tests/compile-pass/question_result.zeta` + `tests/compile-fail/question-nonoption.zeta` + 全量 23 用例全绿 + clippy 0 警告）
- [x] **H2 无捕获闭包**（§3.8 闭包行勾销；guide.md §13 更新）：`|x, y| expr` desugar 为匿名函数 + 函数指针，零运行时开销（typecheck `check_closure_expected`：按预期 fn 签名取参数类型、`std::mem::take` 清空变量环境实现无捕获隔离、body 尾部表达式兼容返回类型、注册 `fn_signatures` + `ctx.mono_items` 注入 `HirItem::Fn(__closure_N)`，复用 H1 函数指针全链路）。三处接线：`check_call` 与 `check_indirect_call` 实参循环（形参 `Type::Fn` + 实参 `ExprKind::Closure` → 预期签名检查）、`check_stmt` Let 分支（fn 注解 + 闭包 init）；捕获检测：body 引用外部变量（`UndefinedVariable`）改写为 Unsupported「闭包捕获外部变量（H3 规划）」。MVP 约束：参数无类型注解需 fn 上下文、仅标识符/`_` 参数模式、不支持返回闭包的函数（可 `let f: fn(..) = |..| ..; f` 转接）。产出 `tests/run-pass/closure.{zeta,out}`（7 输出）+ `tests/compile-pass/closure.zeta`（含 fn 参数闭包 `twice`）+ `tests/compile-fail/closure-capture.zeta`（捕获报错断言）+ 全量 20 用例全绿 + clippy 0 警告）
- [x] **H1 函数一等值（函数指针）**（§3.8 新增特性小节；§3.9 运算符行勾销函数指针；guide.md §13 更新）：`fn(T) -> R` 函数类型 + `let f = add` 函数值绑定 + `f(args)` 间接调用全链路（parser → typecheck `Type::Fn` 签名 + `infer_expr` 函数值推断 + `check_indirect_call` 兜底 → HIR `HirExpr::FnPtr`/`CallIndirect` → MIR `MirStmt::CallIndirect` → LIR → LLVM：函数指针统一存 `i8*` 槽，存储前按签名 `bitcast i64(...)* @fn to i8*`，调用时 `bitcast` 回 `{ret}({params})*` 后间接 `call`）。函数值支持：作实参传递、作返回值（`choose`）、重新绑定、显式类型注解。**关键修复**：① MIR DCE 活跃性收集漏算 `CallIndirect` 实参（`remove_dead_assignments` 缺分支导致实参临时赋值被误删 → 未初始化栈读取产生非确定性垃圾值；`clang` 直接编译 IR + `/tmp/fp_min.ll` 纯 LLVM 最小复现逐层排除）；② MIR inline pass 缺失 `CallIndirect` 分支（内联后整个间接调用指令被丢弃，返回值临时从未写入）；③ borrowck/regionck 补 `HirExpr::FnPtr`/`CallIndirect` 分支；④ `mir_lower_test.rs` match 补 `call_indirect` arm。产出 `tests/compile-pass/fn_ptr.zeta` + `tests/run-pass/fn_ptr.{zeta,out}`（5 输出精确对比）+ 全量 17 用例全绿 + cargo test 全绿 + clippy 0 警告）
- [x] **L2 `serde` 序列化模块**（§3.12 新增特性小节 + §3.9 并发行更新；`docs/grammar.md` 顶部 + PostfixOp turbofish EBNF；guide.md §9.4 新增小节 + §13 更新；std-lib.md §9 状态标注 + §9.1 JSON 小节；mvp-gaps-plan.md §4）：`json::stringify` / `json::parse::<T>` 编译器内建，**零新增 IR 节点**（typecheck 期 desugar 为 String 构建/解析表达式）。**turbofish**：parser `check_turbofish()`（Colon+Colon+Lt 三 token 前瞻）→ 路径解析循环 break → Call 后缀 `::<T1, T2>(args)` 类型实参；`ExprKind::Call` 新增 `type_args: Vec<AstType>` 字段（更新全部构造点/模式匹配：parser/typecheck/fmt/check）。**stringify**（`check_json_stringify` + `json_serialize_ast`）：i64→`int_to_string`、bool→if 三元、String/&str→`"`+`json_escape`+`"`（转义 `"`(34)/`\`(92)/换行(10)/制表(9)）、数组→静态展开、struct→字段序=定义序（嵌套递归）、Vec→while 循环 push_str。**L2f HashMap 序列化**：`Type::Named("HashMap")` 分支——substitute 键/值类型（非 Infer 校验）→ K 限 i64/String → 块表达式 `String::new()` + `{` + `for (k, v) in m`（`check_for_hashmap` 容量扫描、`first` 标志控逗号）→ 键序列化（i64→`"`+int_to_string+`"`、String→json_escape 带引号）+ 值递归 + push `}`。**关键修复**：① `Vec` 是 std struct（data/len/cap），struct 分支置于 Vec 分支后 + guard 排除（否则 `Vec<i64>` 误序列化为 `{"data":[],"len":3,"cap":3}`）；② `vec![...]` 绑定后元素为 `Vec<Infer>`，须注解定型（与 `for x in v`/`check_for_vec` 一致）；③ LLVM 字符串常量 `"` 用十六进制 `\22`（`\"` 被 clang 误解析为 `[1 x i8]` 长度不匹配，更新 `gen_escape_string` 单测）；④ **map! 宏 String 键缺陷**（字面量键无 `String::from` 运行时崩溃）与 **unify 多类型参数错误定型**（substitute 后 Infer 被统一为首实参类型）——`check_method_call` 改按 `raw_params` 逐名绑定（含 Str→String 定型升级）。**parse**（`check_json_parse` + `json_parse_ast`）：缺 `::<T>` 报错；i64→`string_to_int`、bool→字节比较、String→`json_unescape`；MVP 直接返回 `T`（非法输入给默认值 0/false/空串）。**L2g HashMap 反序列化**：`Type::Named("HashMap")` 分支——K 限 i64/String、V 限标量（嵌套 HashMap 显式 Unsupported，`split(",")` 无法处理内层逗号）；desugar 为块表达式：`String::from(arg)` → `substring(1, len-1)` 剥离 `{}` → `split(",")` 分段 → `for x in vec`（`find(":")` 分键值 → 键 `json_unescape`（i64 再 `string_to_int`）+ 值递归 → `__m.insert`），`let __m: HashMap<K,V>` 注解定型（空 `{}` 亦定型）；parser turbofish 嵌套泛型 `>>` 拆分（`pending_gt` 复用 `close_generics`，`::<HashMap<i64, i64>>(s)`）。std：core.zeta 新增 `json_escape`/`json_unescape`（局部变量避开 `out` 保留字用 `buf`；恢复 sed 误删的 `let mut result = 0;`）。产出 `tests/run-pass/json_serde.{zeta,out}`（28 输出：标量/数组/struct/Vec/HashMap stringify（i64·String 键、空 map、嵌套 Vec 值）+ parse i64·bool·String·HashMap（多键 get/缺失键/空对象/String 键·bool 值/String 值）+ stringify→parse 往返）+ parser 单测 `test_turbofish_nested_generics_shr_split` + 全量 43 用例全绿 + cargo test 全绿（678 通过 0 失败））
- [x] **L1 普通函数 `async fn`/`await`**（§3.9 并发行勾销；guide.md §9.3 新增小节 + §13 更新；`docs/grammar.md` 顶部状态标注；std-lib.md §10 状态标注）：MVP 同步语义，**零编译器代码改动**——`async fn` 关键字经 parser（`AstFn.is_async`）解析后由 typecheck 忽略（编译为普通同步函数）；`expr.await`（`ExprKind::Await`）typecheck 直接 `infer_expr(inner)` 求值（`.await` 为语法标记）。抽象复用 actor 异步机制（actor `.await` = ask 同步往返），普通函数同构：`async fn foo(x) -> T` ≡ `fn foo(x) -> T`、`foo().await` ≡ `foo()`；支持嵌套 await、String/数组返回、表达式中间 await。产出 `tests/run-pass/async-fns.zeta` + `.out`（6 输出：直接调用 / `.await` / 嵌套 / String `.len()` / 数组索引 / 表达式中间）+ 全量 42 用例全绿。边界：actor 类型不可作普通函数参数（既有行为）；`Future`/executor 仍为 std-lib §10 规划）
- [x] **S1c `async fn` 状态机**（§3.9 并发行 L1→S1c 更新；guide.md §9.3 重写 + §13 更新；std-lib.md §10 摘要 + §10.3 新增小节；mvp-gaps-plan.md S 阶段/S1c 任务行/验收更新）：**真实状态机 desugar，替代 L1 同步语义**。新 crate `crates/zeta-desugar`：`analyze.rs`（段切分：await 位置识别（let 绑定 / ExprStmt / return / 尾表达式）+ AwaitTarget（AsyncFnCall 直接调用 / IdentVar 带类型注解变量）+ 跨 await 变量提升 + 数据依赖拓扑排序）+ `generate.rs`（`struct __Fut_N`（state + 参数 + 提升字段 + 子 future 槽 zeroing）+ `impl Future for __Fut_N`（poll 状态机）+ 构造器 `fn N(args) -> __Fut_N`）。状态编号：await 段 `k` 占 `2k`（首轮询：`self.__fut_k = f(args)` + poll）/ `2k+1`（恢复轮询：仅 poll 已存槽）；Ready 推进 `2k+2`、Pending 保存 `2k+1` 返回 `Poll::Pending`；尾段状态 `2k` 返回 `Poll::Ready(尾值)`；兜底 `Poll::Ready(0)`。MVP 限制：async fn 参数 `i64`、返回 `i64`/`()`、顶层非泛型无递归；await 仅限直线代码（控制流块内 / 表达式嵌套 await 显式报错）；IdentVar await 初始化仅可引用参数。**关键修复**：① poll 接收者 `&mut self` 须为 `Ref(Path("Self"), true)`（与 parser 一致，`Self` 有 codegen 特判）；② 状态编号 `state_if_await`/`resume_if`/`ready_block`/`pending_block` 原用 `k`/`k+1`/`k+2`，多 await 段错位（段 1 首轮询误为 `state==1` 与恢复段冲突 → 跳段轮询零值 future 或落兜底 0）——统一改 `2k`/`2k+1`/`2k+2`；③ if 语句形态须 `Semi(If)`（与 parser 对 if 语句的解析一致，`Expr(If)` 走尾表达式路径）；④ 手写 `__Fut_f` 对照（`tests/run-pass/async_await.zeta` 输出 43）与 desugar 生成逐节点同构验证。产出 `tests/run-pass/async-fns.{zeta,out}`（重写为状态机语义：无 await 单尾段 42 / 嵌套 await 21 / 多 await 链 + 跨段变量 18 / 语句形式 await + return await 40 / 手动 Pending 恢复跨 await 199）+ `tests/run-pass/manual_state_machine.zeta`（手写状态机示例，`&mut Self`→`&mut self`）+ 全量 86 用例全绿）
- [x] **I3 集合宏 `arr!`/`vec!`/`map!`**（§3.9a 新增特性小节 + §3.9 宏调用行勾销；`docs/grammar.md` §2.14 集合宏 EBNF；guide.md §13 更新）：parse 期 desugar，零新增 IR 节点。`arr![a, b]` → `ExprKind::ArrayLit`；`vec![a, b]` → 块表达式 `let mut __vec_N = Vec::with_capacity(n); __vec_N.push(a); ...; __vec_N`（`map!` 同构：`HashMap::with_capacity(n)` + `.insert(k, v)`，元素须 `k => v`）；空集合 → `Vec::new()`/`HashMap::new()`。实现于 `parse_macro_call` 的 `is_collection_macro` 分支 → `parse_collection_macro`（`crates/zeta-parser/src/expr.rs`）：元素 token 流包裹 `(` `)` 后经子 `Parser::from_tokens`（继承宏注册表，支持嵌套宏调用与完整表达式）解析——`arr!` 走 `parse_call_args`，`vec!`/`map!` 先 `expect(LParen)` 再逐元素循环（`parse_expr` + 可选 `FatArrow` 对 + `Comma` 分隔 + `check(RParen)` 空集合短路 + `expect(RParen)` 收尾）；Parser 新增 `collection_temp_seq` 计数器生成 `__vec_N`/`__map_N` 临时变量。**关键教训**：子 Parser 手动元素循环必须先消费包裹的 `(`——`parse_call_args` 内部自消费，照搬会在 `vec![]` 处把 `(` 当分组表达式解析触发 UnexpectedEof（空 `( )` 组）。**零下游改动**：`Vec::with_capacity`/`HashMap::with_capacity` 为 typecheck 既有构造器特判（`resolved.split_once("::")`），`push`/`insert` 为 std 方法查询，块语义复用现有 desugar。产出 `tests/run-pass/collection_macros.{zeta,out}`（10 输出：arr! 索引 / vec! 带元素·空·嵌套宏元素·绑定变量算术元素 / map! 键值对·get·空·键冲突覆盖 / vec! 结果可变 push）+ 全量 41 用例全绿 + cargo test 全绿 + clippy 0 警告）
- [x] **宏系统：声明式宏 + 内置格式化宏**（§3.9 宏调用行勾销；`docs/grammar.md` §2.14 实现标注；guide.md §13 更新）：新 crate `crates/zeta-macro`（matcher/transcriber token 解析、匹配 `$x:expr`/`ident`/`ty`/`tt` 四类元变量 + `$(`...`)` 重复 `*`/`+`/`?`，`MacroError(String)`）；lexer 新增 `$`/`?` token（`Dollar`/`Question`，`!` 保留 `not` 一元运算符语义）；parser 集成——`macro_rules!` 定义注册（`parse_macro_rules` + `collect_group_content` 深度计数定界组收集，`AstMacroDecl` 占位节点）、`name!` 调用（内置宏 → `MacroCall` AST；用户宏 → `expand_macro` 递归展开 + 子 Parser `from_tokens` 再解析，`MAX_MACRO_DEPTH=64` 防爆栈、多余 token 报错）；AST 新增 `ExprKind::MacroCall`；typecheck 格式化引擎——`parse_format_string`（`{}` 值占位 / `{:?}` 同构 / `{{` `}}` 转义 / 占位符数量检测）+ `value_to_string_for_ty`（i64→`int_to_string`、bool→If 三元、String/&str/Str→`String::from`）+ `check_format_macro`（`+` 拼接复用 A3 clone+push_str 语义 → 内建 `println`）+ `check_dbg_macro`（`let __tmp` + println + 返回 `HirExpr::Block`）。**关键修复**：① matcher 路径 `expect(LParen)` 提前消费导致 `collect_group_content` 双重 bump 吞掉首个 `$` token（移除预消费）；② `dbg!` 先推断参数类型再按预推断类型构造 String 转换（`value_to_string_for_ty` 拆分），避免未绑定 `__tmp` 报 UndefinedVariable。产出 zeta-macro 12 单测 + `tests/compile-pass/macro.zeta` + `tests/run-pass/macro.{zeta,out}`（输出精确对比）+ 全量 116 套件全绿 + clippy 0 警告）
- [x] **G2 `str` 切片与 String 补齐**（§3.9 `String::from` 行勾销 + 引用类型行更新；详见 [`docs/mvp-gaps-plan.md`](docs/mvp-gaps-plan.md) §4）：`String::from` 运行期内容（String 变量 → `s.clone()` 深拷贝；`&str` → 读 data/len 槽深拷贝；消除 §13 约束 4）+ `&str` 只读借用视图（`String::as_str()` → `HirExpr::Ref` 对象指针拷贝，运行时 = 指向 String 对象的瘦指针，复用 G1 聚合引用机制，MIR/LIR/codegen 布局零改动；`resolve_ast_type` 识别 `str` 类型关键字；方法/索引/切片对 `&str` 归一为 String 处理；`compatible_with` 允许 `&str` ↔ `&String` 互视与内容比较；`comparison.rs` 字符串比较纳入视图）。**设计决策**：`substring` 保持拷贝返回（避免破坏现有 API），零拷贝以 `as_str()` 借用视图体现。产出 `string_from_runtime_test.rs` 6 用例 + `str_ref_test.rs` 9 用例 + 全量 114 套件全绿 + clippy 0 警告）
- [x] **G1 引用类型与表达式**（§3.9 引用类型行勾销；详见 [`docs/mvp-gaps-plan.md`](docs/mvp-gaps-plan.md) §4）：`&T`/`&mut T` 类型 + `&x`/`&mut x`/`*` 表达式全链路（parser → typecheck → HIR → MIR → LIR → LLVM）。标量引用存 `i8*` 值槽（`*p` 读写按标量种类 bitcast）、聚合引用拷贝对象指针；字段访问/方法调用自动 `peel_ref`（`p.x`/`v.len()` 免显式解引用）；`&mut T` 兼容 `&T` 参数（宽松规则，严格可变性互斥留给 borrowck）。关键修复：MIR inline 补 AddrOf/DerefRead/DerefWrite 映射、DCE 活跃性纳入 Deref* 读、codegen 指针拷贝 `%%` 转义、**顶层同名遮蔽**（用户 `fn read` vs std extern `read`：用户侧注册 `read@shadow<N>` mangle 名、std 原名保留，std 模块内部裸名绑定 extern 原版、用户顶层绑定自身版本；签名相同的重声明如 `extern fn __zeta_target_os()` 保留原名）；`reference_test.rs` 10 用例 + 全量 112 套件全绿）
- [x] **标准库模块化拆分**（`zeta-std/zeta/core.zeta` 1323 行单文件 → 根模块 + 四个子模块文件：`time.zeta`（Duration/Instant）/ `io.zeta`（文件 IO/控制台 IO）/ `net.zeta`（socket/字节序/主机名）/ `sync.zeta`（pthread 锁）；driver `stdlib.rs::load_std_prelude` 改为复用 `load_combined_source` 模块展开（`mod io;` → 内联块，相对 `core.zeta` 所在目录解析）后注入，用户侧 API 不变；**约束**：① 编译器按全名特判 String/Vec/HashMap 构造器与 `alloc_bytes` 等 callee 名，② extern 的 LLVM `declare` 符号必须与 libc 一致（模块前缀会改名致链接失败）——故 String/Vec/Option/Result/HashMap、内建函数与**全部 extern 声明**留在根，子模块仅含自由函数与普通 struct；③ 兼容经 core.zeta 末尾 `mod time; mod io; mod net; mod sync;` + `use time::Duration; use io::read_file; ...` 重新导出（裸名即用，mod 声明置于文件尾符合类型顺序解析）；④ 子模块内自引用类型用全限定名（`time::Instant` / `sync::Mutex`，模块内裸名类型解析无前缀回退），配合 `check_struct_construct` 支持 use 别名/模块路径解析（1 处编译器修改，`resolve_full_name` 兜底）；io/net/time/sync/hashmap/std_prelude 子集测试 + 全量 111 套件全绿 + 跨模块冒烟（io 文件读写 + Duration 换算 + Mutex/RwLock + HashMap + 时间字面量））

- [x] **F2 PGO 数据回灌（编译流程）**（`.zeta_profile` → 区域大小预测闭环：zeta-driver 接入 `zeta-region-alloc`（workspace 已注册，补 re-export `PgoAdvisor`/`CompilerInterface`/`RegionCompileInfo`/`ProfileCollector`）；新命令 `zeta profile <file.zeta_profile> [--out <report.md>]`（`run_profile`，加载 PGO 画像 JSON → `PgoAdvisor::recommend_size`（p95×1.1，下限 64KiB）→ `CompilerInterface` 报告：每区域 estimated=历史均值 / initial=PGO 建议 / max=建议×4 与峰值取大 / decision 附 p50/p95/mean/max 依据，区域按 ID 排序）；`zeta build --profile <file>` 编译期注入（构建完成后打印预测报告，加载失败仅告警不阻断构建——profile 为可选优化输入）；lib.rs 新增 `region_profile_report(path)`（文件读取 + 解析，`DriverError::Profile` 承载 JSON 错误）+ `build_region_report(data)`（纯函数可单测）；`profile_cmd_test.rs` 6 用例：报告元数据/建议与 advisor 一致 + 下限回退（大样本 p95×1.1>64KiB 采纳、小样本回落 64KiB）/文件往返/坏 JSON [profile] 错误/缺文件 I/O 错误/空数据 "(none registered)"；冒烟：手写 `.zeta_profile` → `zeta profile` 输出 worker_pool initial 858000（780000×1.1）与 tiny_region 65536（下限），`zeta build --profile` 编译完成 + 报告注入；语言级 region 接线后预测可直接回灌 `region 'r adaptive` 初始容量）
- [x] **遗留修复：动态切片 + `zeta new` + `String::from` 字面量变量**（`Vec<T>` 动态切片 `v[lo..<hi]`/`v[lo...hi]`/`v[lo<..hi]`：std 泛型方法 `Vec::slice`（越界 clamp 到 [0, len]、start>=end 空、返回全新缓冲，元素按值拷贝）+ `check_slice` 三路分支实例化（String→substring / Vec→slice / 数组→展开）；数组 `[T; N]` 动态切片：typecheck 展开为 Vec 拷贝循环（`let __base` 绑定防重复求值 + Vec::new cap 4 三槽构造 + 计数器/上界 + 边界 clamp 到 [0, N] + `while __i < __hi { __out.push(__base[__i]); __i += 1 }`，push 经 `instantiate_impl_method` 泛型实例化，支持 i64/f64/bool/u8 元素）；`String::from(s)` 支持绑定字面量的变量（`TypeContext.local_inits` 表记录 `let` 初始化表达式，`check_string_from` 对 `Variable` 追踪回字面量；非字面量 Str 的长度表达仍待实现）；`zeta new <name> [--lib]` 命令（委托 zep `cmd_new` 生成 Zeta.toml + src/main.zeta|lib.zeta）；`dynamic_slice_test.rs` 7 用例全绿 + 全量回归 111 套件）
- [x] **F1 LSP 服务器（MVP）**（新 crate `crates/zeta-lsp` + `zeta lsp` CLI 命令：JSON-RPC 2.0 over stdio（Content-Length 帧，自研轻量 `jsonrpc.rs` 无外部 LSP 依赖）+ full 文本同步（didOpen/didChange/didClose）+ 诊断推送（复用 zeta-check 静态分析：parse-error 与 unused-variable 等 lint 规则映射为 LSP Diagnostic，0 起始 range/severity/source/code，行覆盖锚点）；`initialize` 声明 capabilities（textDocumentSync=1），shutdown/exit 生命周期，未知请求 -32601；`server.rs::handle` 为纯函数入口便于单测（11 单测：帧编解码/生命周期/诊断映射）+ `lsp_e2e_test.rs` 2 进程测试（spawn `CARGO_BIN_EXE_zeta-lsp` 完整协议往返：initialize→didOpen 告警→didChange 语法错误→didClose 清空→shutdown→exit）；冒烟 `zeta lsp` 返回 capabilities；zeta-driver 全量回归绿；MVP 诊断仅覆盖语法 + lint（typecheck 级诊断后续迭代））
- [x] **A3 String 拼接拷贝语义**（消除共享缓冲别名隐患：原 `a + b` desugar 为 `let __s = a; __s.push_str(b)`——3 槽值拷贝共享 data 缓冲，拼接结果与左操作数互相污染（实测：`s.push_str` 使 `a` 变长、`a` 扩容后 `s` 内容被改）；修复为 `let __s = a.clone(); __s.push_str(b)`：std `String::clone()` 深拷贝（`String::new()` + 逐字节 `push_byte`，复用 substring 模式）+ typecheck desugar 改为调用 clone；`string_concat_test.rs` 9 用例（原 6 + 新增 result_isolation/lhs_isolation/clone_direct），`concat_grow` cap 预期 24→16（clone 从 cap 0 起 0→8→16 翻倍））
- [x] **用户级 match 解构具体实例化枚举聚合载荷**（修复 `expected String, found T`：`check_pattern` Enum 分支原仅用 `ctx.generic_subst` 替换字段类型，用户级 match（非泛型方法体）subst 为空导致 `T` 未定型；修复为合并「当前 generic_subst + 由 `pat_ty` 类型参数与 `enum_def.type_params` 建立的新映射」，嵌套泛型 `Option<Vec<T>>`/`Result<Option<String>, i64>` 递归生效；`option_result_test.rs::user_level_match_string_payload` 8 断言覆盖 Some/None/Ok/Err/嵌套/方法链）
- [x] **Infer 枚举自动定型**（修复裸 `Result::Err(7).unwrap_or(100)` 返回 `_`：`check_method_call` 参数检查时对含 `_` 的期望类型用实参 unify 回填 subst（`unify` 新增 `Type::Infer` 分支：替换 subst 中所有 Infer 条目），回填后重算签名再实例化方法；`option_result_test.rs::bare_enum_infer` 5 断言覆盖裸 Ok/Err/Some + 聚合载荷 + 算术链）
- [x] **通用 FFI：`extern fn` 声明**（打通 parser→typecheck→HIR→MIR→LIR→LLVM→链接全链路：AST `AstFnDecl.is_extern` + parser 识别 `extern` 前缀；typecheck 允许无 body 并序列化签名（`type_to_extern_name`，标量 i8/i16/i32/i64/isize/u8/u16/u32/u64/usize/f32/f64/bool/char/()/&T）；HIR `HirFnDecl.is_extern` + `extern_sig`；MIR `MirFunction.is_extern` + `extern_sig`（空 CFG，DCE pass 跳过）；LIR `LirFunction.is_extern` + `parse_extern_type`（未知名→Ptr），extern 跳过类型推断/重算循环；codegen 对 extern 生成 `declare` 而非 `define`（`llvm_global_name`）；`ffi_extern_test.rs` 3 用例：i64（libc labs）/f64+void（fabs、srand）/嵌套+循环+运算（labs、llabs）——extern 符号由链接器解析，为 io/net 绑定层接线扫清障碍）
- [x] **`Duration`/`Instant` 时间模块**（core.zeta 末尾；底层时钟 libc `clock()` extern（POSIX CLOCKS_PER_SEC=1e6 微秒）；`Duration { micros: i64 }` 结构体 + `secs()`/`millis()`/`micros()`/`nanos()` 单位换算（整除/乘法）；`Instant { start: i64 }` + `now()`（关联函数，`Instant { start: clock() }` 字面量构造）/`elapsed()` 返回 `Duration`（时钟差）；首例 extern 驱动标准库模块，验证 FFI 全链路可用；`time_test.rs` 3 用例：单位换算（含非整秒/零值）/字段访问+算术/now-elapsed 时钟差非负+计算执行）
- [x] **`Vec<T>` 补充方法：`first`/`last`/`reverse`/`swap`/`binary_search`**（core.zeta Vec impl；`first`/`last` 空容器返回 `Option::None`、否则值拷贝 `Some(self.data[0])`/`self.data[len-1]`；`reverse` 双指针原地交换 O(n)；`swap(i,j)` 越界 `loop{}` 崩溃替代；`binary_search(x)` 三态二分（`data[mid] < x` / `x < data[mid]` 双比较区分，`<` 实例化后按具体类型比较——i64 数值/String 字典序），命中返回下标（最左）、未命中 -1；`vec_more_ops_test.rs` 5 用例：first/last 空+多元素/单元素、reverse+swap 原地验证、binary_search i64 边界+未命中、binary_search String（含 sort 组合）、组合链路）
- [x] **`HashMap` `len`/`is_empty` 确认已有**（`len(&self)`/`cap`/`is_empty(&self)->bool`/`contains_key` 均已在早期实现；本轮补充 `hashmap_len_test.rs` 4 用例固化：空表+插入、同键覆盖不重复计数+remove 墓碑递减、扩容 rehash 后 len 保持+clear 归零+复用、String 键实例化；发现并删除了一次重复的 `len`/`is_empty` 定义）
- [x] **io 模块（B2）**（core.zeta；基于 extern FFI 的 libc stdio 文件 IO：`fopen`/`fread`/`fwrite`/`fclose`/`fseek`/`ftell` + POSIX `read`（fd 0 = stdin）；`c_str` NUL 结尾拷贝（`String::with_capacity(len+1)` + push_byte(0) + 恢复 len）+ `read_file`（fseek SEEK_END + ftell 取 size + 单次 fread 读满 + 手动设 len）/`write_file`（"w" 截断）/`append_file`（"a" 追加）/`read_line`（stdin 读一行，fd 0）；**选 stdio 而非 open()**：mode 字符串 "r"/"w"/"a" 与 SEEK_SET/SEEK_END 在 C 标准层可移植，避开 open() 平台相关 flags（macOS O_CREAT=0x200 vs Linux 0x40）；**编译器改动**：extern `String` 参数在 LIR 登记为 `Str`（`parse_extern_type` 新增 `"String" => Str`），codegen sigs 增加 is_extern 标记（`HashMap<String,(Vec<LirType>,LirType,bool)>`），`emit_call` 对 extern 的 `Str` 参数生成 `bitcast 结构体指针 → load 槽 0 data 指针` 传给 libc（String 局部变量存的是结构体指针，C 侧需要 data 指针；普通函数调用不受影响）；FILE* 以 i64 承载（x86-64 ABI 指针与整数同宽）；`io_file_test.rs` 9 用例：往返+字节数（Hello Zeta! = 11 字节）/覆盖截断/追加/UTF-8 中文 16 字节往返/4096 大文件（len+首尾字节）/缺失文件空串/空文件/组合链路（写→读→拼接→再写→读）/hostname）
- [x] **net 基础绑定（B3 部分）**（core.zeta；`gethostname` extern + `hostname()`——不依赖返回值（`gethostname` 返回 int 以 i64 声明时高位未定义），仅扫描缓冲内首个 NUL 定位实际长度（无 NUL 回退 255））
- [x] **位运算全链路（`&`/`|`/`^`/`<<`/`>>`）**（解除 B3 剩余阻塞的核心能力：parser/AST 早已解析，typecheck 原以 placeholder `HirBinaryOp::Mod` 显式报 `Unsupported`「位运算在 MVP 阶段」且 HIR/MIR/LIR/codegen 全未实现；本轮打通：HIR `HirBinaryOp` 新增 5 变体、typecheck 真正映射（整数操作数，结果取左操作数类型）、MIR const_fold 折叠（`wrapping_shl/shr` 位移量截断 u32 兜底 poison）、LIR `binary_result_type`/操作数类型推断统一 I64、codegen 生成 `and`/`or`/`xor`/`shl`/`ashr`（右移用**算术 ashr**保持有符号语义）；`bitwise_test.rs` 6 用例：基础运算（常量折叠 + 变量两路径）/移位含负数/优先级混合（`*`>`+`>`<<`>`&`>`^`>`|`，`COMPARE > BIT_AND` 故裸 `x & 3 == 2` 解析为 bool 参与位运算需括号）/字节打包解包（端口 8080、IPv4 0xC0A8010A）/掩码应用/循环累积移位（字节流拼接 0x010203））
- [x] **net 模块（B3 完成）**（core.zeta；位运算打通后启用：`socketpair`/`socket`/`connect`/`close`/`r#send`/`r#recv`（`send`/`recv` 为 actor 保留字，用原始标识符 `r#` 绕开）extern + `htons`（纯位运算字节交换）/`socketpair_stream`（AF_UNIX SOCK_STREAM 全双工，fd 数组承载于 `String::with_capacity(8)`）/`fd_at`（小端 int32 字节解释）/`send_all`/`recv_some`/`sockaddr_in4`（macOS 布局：sin_len 头 + AF_INET + 端口/IP 大端逐字节打包，**直接按大端拆字节而非经 htons**——htons 返回交换后的数值再拆会反向；已由 `__zeta_target_os` 平台内建双布局化，见 E1）/`tcp_connect`（socket→sockaddr→connect，失败 close 释放返回 -1）；socket/connect/close 返回 int 以 `-> i32` 声明（复用 B4 extern_ret32 清洗），send/recv 返回 ssize_t（i64）；`net_socket_test.rs` 6 用例：htons 字节序（含往返对合）/socketpair 单双向字节流/fd 数组解析/循环多包/4096 二进制块往返（循环收满防 SOCK_STREAM 分片）/sockaddr_in4 字节布局（sin_len=16、AF_INET=2、8080→[31,144]、127.0.0.1）/connect 未监听端口 127.0.0.1:1 拒绝（ECONNREFUSED 而非 EINVAL/EFAULT——验证构造的 sockaddr_in 被内核正确解析）；Linux 无 sin_len 字段——已由 E1 平台内建 `__zeta_target_os` 双布局化（`sockaddr_in4_with_layout`：macOS sin_len 头 / Linux family 小端），消除「待 cfg 支持」遗留）
- [x] **sync 模块（B4）**（core.zeta；基于 extern FFI 绑定 pthread：`Mutex { p: i64 }` + `new`/`lock`/`unlock`/`try_lock`、`RwLock { p: i64 }` + `new`/`read_lock`/`write_lock`/`unlock`/`try_read_lock`/`try_write_lock`；原语对象承载于 `calloc` 缓冲（`pthread_mutex_t` macOS 64/Linux 40 字节、`pthread_rwlock_t` macOS 200/Linux 56 字节，统一 `calloc(1, 256)` 保守分配；**选 calloc 而非 malloc**：内建 alloc_array/alloc_bytes 已声明 `i8* @malloc`，extern 以 i64 声明会触发 LLVM redefinition；无显式释放，MVP 无析构，进程退出时 OS 回收）；Condvar/Barrier 骨架声明留注释（MVP 无函数指针无法 `pthread_create`，wait/signal 与 count>1 语义需多线程）；**编译器改动**：extern 返回 `i32` 支持（LIR `LirFunction.extern_ret32` 标记 + `parse_extern_type` 加 `"i32"` 映射；codegen sigs 表第 4 字段 + `declare i32` + 调用点 `sext i32` 存槽——根治 int 返回 extern 高位未定义问题，`trylock` 等返回值可靠）；`sync_test.rs` 6 用例：Mutex trylock EBUSY 互斥语义/临界区计数/百次循环、RwLock 读读共享+读锁下写锁 EBUSY/写锁排他/读写交替循环）
- [x] **修复内联 pass 变量重命名 bug**（zeta-mir/passes/inline.rs；原实现仅对 `_` 开头临时变量重命名，被内联函数内**用户命名的局部变量**（如 `let r = ...`）原样保留，内联进调用者后与同名变量合并为同一槽位导致语义错误——`r.try_read_lock()` 的返回值被写回调用者 `r`，第二次调用把 0/1 当结构体指针解引用崩溃；修复为 `map_local` 对非参数名字一律重命名（`_i{seq}`），参数仍经 subst 映射到实参）
- [x] **阶段 C：Actor 语言级接线（C1–C3 全部完成）**（`crates/zeta-driver/tests/actor_test.rs` 8 用例全绿；详见 [`docs/development-plan.md`](docs/development-plan.md) 阶段 C 执行记录）
  - **C1 actor desugar 全链路**：`actor` 语法经 `zeta-typecheck` `expand_actor` 展开为普通结构 + 生成函数 `<Actor>::__state_new`（N 槽 calloc 缓冲 + 字段初值）/`__m<i>`（第 i 个方法：self 槽数组 + 消息槽参数）/`__handle`（按 kind 分发，槽 4/5 为 kind 与返回槽）/`__handle_message`（返回槽 5 结果）；runtime `CallbackActor` 承载（状态指针 + handler fn 指针，u64 槽语义）；方法返回 -1 = 崩溃信号（`u64::MAX` → Panic）；编译器自动生成 `zeta_actor_spawn`/`spawn_supervised`/`ask`/`send` extern 声明（**用户显式声明则跳过生成**）
  - **C1 排障**：① 返回 -1 误触崩溃协议 → Panic 前先 `reply(0u64)`（裸字面量推断 i32 会 WrongReplyType），否则 ask 干等满 ASK_TIMEOUT ② supervisor 重启竞态：崩溃时旧 state 放回 + `running=false` 使后续消息被其他 Worker 用旧 state 处理 → 崩溃时保持 `running=true`、不放回旧 state，重启完成后重置并重新调度 ③ staticlib 陈旧：改 runtime 后需显式 `cargo build -p zeta-actor-runtime`
  - **C2 语言级受监督 spawn**：`Actor::new_supervised(strategy)`（0=OneForOne 1=AllForOne 2=RestartForOne）→ `zeta_actor_spawn_supervised("<__handle>", "<__state_new>", strategy)`（factory 传符号名，runtime 内部 dlsym）
  - **C2 修复 `String::from` NUL 终止 bug**：原展开 `alloc_bytes(len)`+`copy_bytes(len)` 缓冲末尾无 NUL，runtime `CStr::from_ptr` 按 NUL 扫描读超界（16 字节 `Wobbly::__handle` 读到相邻堆垃圾，17 字节 `Counter::__handle` 靠对齐运气幸存）→ `alloc_bytes(len+1)`+`copy_bytes(len+1)`（LLVM 字符串常量自带 `\00` 拷入），cap 字段保持 len
  - **C3 示例与测试固化**：`examples/actor-ping-pong.zeta`（ask 往返 + send 异步 + FIFO，输出 11/12/2/4）、`examples/actor-supervisor.zeta`（`new_supervised(0)` 崩溃恢复，输出 5/0/3）；集成测试补 `crash_without_supervisor_stops_actor`、`send_fifo_order`
  - **D1 `zeta test` 子命令**（`crates/zeta-driver/src/test_runner.rs`：`run_test_suite(root)` 扫描 `compile-pass/`/`compile-fail/`/`run-pass/` 三个子目录——compile-pass 编译到 LLVM IR 成功即过；compile-fail 要求编译失败，源内 `// expect: <片段>` 注释断言错误消息包含片段；run-pass 编译运行成功，同名 `.out` 文件作为期望 stdout 精确对比；用例目录可选、文件按名排序保证确定性；CLI `zeta test [<tests-dir>]` 默认 `./tests`，逐用例打印通过/失败 + 汇总 + 失败时非零退出码；**cargo 矩阵**：`crates/zeta-driver/tests/suite_test.rs` 断言 `tests/` 全部通过且三类均有覆盖；初始用例 13 个：compile-pass 5（hello/arith/struct-trait/modules/actor）+ compile-fail 5（type-mismatch/undefined-var/unknown-field/undefined-fn/no-method，均带 expect 断言）+ run-pass 3（hello/arith/actor-ping-pong + .out））
  - **D1 连带修复 ① 模块项 pub 可见性**（`zeta-parser/src/parser.rs`）：`parse_item` 的 `Pub` 分支原仅识别 `pub mod`，其余一律按函数解析（`pub const PI` 报 `expected 'fn', found Const`）→ 按实际关键字分派到 mod/const/static/struct/enum/trait/impl/use/actor；`pub fn`（含 async/unsafe/extern 前缀）不预消费 `pub`、交回 `parse_fn` 保证 `is_pub` 正确
  - **D1 连带修复 ② use 导入常量短名解析**（`zeta-typecheck/src/context.rs`）：`lookup_constant` 原仅按裸名查表（`use math::PI; println(PI)` 报 `undefined variable`）→ 裸名失败后回退 `use_aliases`，`resolve_full_name` 增加 `constants` 直接命中分支
  - **D2 `zeta fmt` 格式化器**（`tools/zeta-fmt`）：解析为 AST 后按统一规范重建——顶层项空一行、块类表达式多行展开、表达式按**优先级表**重排括号保证语义不变（Assign<Or<And<BitOr<BitXor<BitAnd<Compare<Shift<Add<Mul<Cast<Unary<Postfix，右侧同优先级补括号）、字符串/字符按 lexer 转义规则重编码（`\"`/`\\`/`\n`/`\xNN`）、浮点字面量强制保留小数点、时间字面量还原（`9am`/`6pm`/`12:00`/`1:05pm`）；`AstStmt::Expr`=带分号语句 / `AstStmt::Semi`=无分号块后语句（与 parser 语义一致）；self 接收者打印 `&self`/`&mut self` 特例；注释暂不保留（MVP 基于 AST 重建）；CLI `zeta fmt <file> [--check] [-w] [--indent N]`（默认 stdout，`--check` 检查是否已格式化 exit 1，`-w` 写回）
  - **D2 `zeta check` 静态分析器**（`tools/zeta-check`）：4 条 AST lint 规则 + parse-error——`unused-variable`（块/函数/闭包/for 作用域栈 + 遮蔽查找，`_` 通配与 self 接收者豁免）、`constant-condition`（if/while 条件为字面量或 `!字面量`）、`redundant-compare`（`==`/`!=` 两侧均为字面量）、`unreachable-code`（return/break/continue 后语句，含 `loop` 体内 break 后）；诊断格式 `line:col: level[rule]: message`；CLI `zeta check <file>`（有诊断 exit 1）
  - **D2 driver 挂载**：`zeta-driver` 增加 `fmt`/`check` 子命令（复用 tools 库，`check_source_file` 仅静态分析不跑流水线）；两个 tools crate 各带独立 binary + 根 workspace 依赖表登记
  - **D2 测试固化**：`zeta-fmt` 9 单测（round-trip 再解析/幂等/顶层空行/优先级括号/浮点小数点/字符串转义/时间字面量/parse-error/actor+region）、`zeta-check` 12 单测（每规则正反用例+遮蔽+参数+self 豁免+干净代码零诊断）、`crates/zeta-driver/tests/fmt_check_test.rs` 5 集成（真实源码格式化 round-trip 幂等、格式化后**编译运行语义不变**、干净代码零诊断、规则报告与行号、parse-error）；`zeta test` 13/13 全绿 + 全量回归通过
  - **D3 `zeta doc` 文档生成器**（`tools/zeta-doc`）：按源码位置提取 `///` 文档注释（连续行合并为块，允许空行/普通注释间隔，紧邻顶层项即关联；`//!` 行作为文件级文档渲染在标题下方；无注释的项仍以签名+位置出现保证 API 目录完整）；输出 Markdown：标题 + 文件级文档 + 目录（分类标签+简短标题）+ 按类别分组正文（函数/结构体/枚举/Trait/impl/Actor/常量/模块/其他），每项渲染「标题、```zeta 签名代码块、`> 位置: line:col`、文档正文」，聚合类型附成员列表（trait/impl 方法、actor 字段与方法、mod 子项）；**签名重建**独立实现（`item_signature`/`fn_signature`/`fmt_type`/`fmt_param`：`pub`/`async`/`extern` 修饰符、泛型参数、self 接收者 `&self`/`&mut self` 特判——`self: &Self` 无法再解析）；CLI `zeta doc <file.zeta> [--out <file.md>] [--title <标题>]`
  - **D3 `zeta bench` 基准测试框架**（`tools/zeta-bench`，纯 std 无外部依赖）：`BenchOptions { warmup, runs, quiet }`（预热轮数不计统计）；`bench_executable` 多次启动进程 `Instant` 计时、非零退出码报错；`bench_source` 接受编译回调先编译再计时；`BenchReport` 输出平均/中位数/最小/最大/样本标准差/吞吐（ops/s）类 criterion 摘要；CLI `zeta-bench <file.zeta|exe> [--runs N] [--warmup N] [--out <路径>] [--quiet]`（源码模式自定位 `zeta build`：复用当前二进制或 PATH）
  - **D3 driver 挂载**：`zeta-driver` 增加 `doc`/`bench` 子命令（`doc_source_file` 仅文档生成不跑流水线；`zeta bench <file.zeta> [-o <out>] [--runs N] [--warmup N] [--cache-dir <dir>] [--force]` 复用 `build_file` 编译后交 `zeta_bench::bench_executable` 计时）；`DriverError` 新增 `Doc` 变体
  - **D3 测试固化**：`zeta-doc` 5 单测（相邻 `///` 合并/`//!` 与 `///` 分离/空白与普通注释间隔关联/代码行阻断/self 签名）、`zeta-bench` 3 单测（统计计算/单样本与空样本/时长格式化）、`crates/zeta-driver/tests/doc_bench_test.rs` 9 集成（doc 注释+签名+位置提取、无注释目录、parse-error、driver 文件 API 正反、bench 统计、产物计时、编译回调链路、缺失产物报错）；`zeta doc` 对真实 `core.zeta` 输出完整目录；全量回归通过
  - **E1 `--target` 交叉编译（macOS 双架构基础）**：编译流水线目标无关（LLVM IR → clang），交叉编译 = 链接阶段注入 triple；`zeta-driver` 新增 `host_triple()`（本机 triple）/`target_arch()`（arm64/aarch64 归一）/`is_cross_target()`；`assemble` 接受 target → clang `--target=<t>`；**Actor 运行时跨架构跳过**（本机 staticlib 无法链接其他架构，stderr 提示）；API：`build_executable_with_target`/`build_executable_file_with_target`/`IncrementalDriver.with_target`；CLI `zeta build <file> --target <triple>`（run 拒绝）；验证：同一源码编译出 arm64 + x86_64 Mach-O，本机与 Rosetta 均运行输出 `Hello, Zeta!`；Windows/ARM 链接器/库路径留待后续（`sockaddr_in4` 布局已由平台内建消除）
  - **E1 测试固化**：`crates/zeta-driver/tests/cross_compile_test.rs` 6 集成（arch/triple 归一化、本机 target 编译运行、无 target 兼容、无效 target 报错、macOS 双架构产物 `file` 校验、std 特性程序交叉编译 Rosetta 运行）；全量回归通过
  - **E1 平台内建 `__zeta_target_os`（消除平台相关假设）**：Zeta 无 `#[cfg]` 属性机制（词法层无 `#`），改由**驱动注入平台内建**：`target_os_code(target)`（triple→OS 码 0/1/2/3/4）+ `platform_builtin_ir` 在 assemble 写盘前注入 `define internal i32 @__zeta_target_os()`；**codegen 对 `__zeta_` 前缀 extern 跳过 declare**（防 declare+define 冲突）；`core.zeta` `extern fn __zeta_target_os() -> i32` + `sockaddr_in4_with_layout(has_sin_len, ...)` 双布局（macOS：0=sin_len(16),1=AF_INET(2)；Linux：0-1=sin_family 小端 0x0002），`sockaddr_in4` 按平台自适应——消除「Linux 无 sin_len 待 cfg 支持」遗留
  - **E1 测试固化**：`crates/zeta-driver/tests/platform_builtin_test.rs` 4 集成（target→OS 码映射含主机、平台内建端到端接线、sockaddr_in4 双布局字节序列、双布局尾部一致性）；全量回归 104 套件通过

---

## 6. 开发里程碑

### Milestone 1 — 编译器 MVP（Month 0-4）

- [x] **M1.1** Lexer 完成（支持全部关键字和运算符）
- [x] **M1.2** Parser 完成（支持完整语法）
- [x] **M1.3** AST → HIR  lowering
- [x] **M1.4** 类型检查器基础（比较链 + `in` 表达式语义 + `for` range 迭代器 desugar；泛型/trait/聚合对象特性已由后续任务补齐）
- [x] **M1.5** 借用检查器（L0 所有权系统：use-after-move + 不可变绑定赋值，区域块值传递例外；由 P012 补齐 16 测试；`&self`/`&mut self` 引用接收者方法已支持）
- [x] **M1.6** 区域系统（`zeta-region-alloc` bump allocator + 四策略 + LIFO 析构 + `execute_transfer` 所有权句柄；`zeta-regionck` 嵌套/归属/transfer 合法性 + P005 嵌套方向/PartialTransfer 语义；MIR 区域指令显式化由 P011 完成）
- [x] **M1.7** MIR + 基础优化 passes（P011：CFG lowering：if/while/loop/break/continue + 区域指令显式化；常量折叠、DCE、小函数内联）
- [x] **M1.8** 代码生成（P013：LIR 三地址码 + LLVM IR 文本后端，x86_64 Linux）
- [x] **M1.9** 能编译并运行 `hello-world.zeta`

### Milestone 2 — 生产可用（Month 4-8）

- [x] **M2.1** Actor 运行时（`zeta-actor-runtime`：工作窃取调度 + 每 Actor 互斥处理 + 有界邮箱 + ask/reply 模式 + Supervisor 监督恢复（OneForOne/AllForOne/RestartForOne + 频率限制）+ 内置 Router/Timer + 优雅排空关闭；`ActorRef`/`Runtime` API 就绪，语言级 `actor`/`spawn` 语法与标准库集成待 M2.5）
- [x] **模块系统**（嵌套 `mod {}` + `mod foo;` 多文件加载（`foo.zeta` / `foo/mod.zeta`）+ `use` 导入与别名 + 扁平符号名 `mod::item` + LLVM 引号标识符接线；typecheck 支持模块内函数/常量/结构体符号解析）
- [x] **M2.2** 包管理器 Zep（P008：pubgrub 依赖解析 + 本地/HTTP 注册表 + 打包解包 + 构建驱动，29 测试）
- [x] **M2.3** 增量编译引擎（源码/接口哈希 + LLVM IR 产物缓存 + 依赖图 + 多文件模块编译缓存）
- [ ] **M2.4** LSP 服务器（IDE 支持）
- [x] **M2.5 部分完成** 标准库核心模块：P009 完成 `Option<T>`/`Result<T,E>` 纯 Zeta 实现 + 编译器标准库搜索路径注入（`--no-std`/缓存键覆盖）+ NIO/sendfile 绑定层；**collections 已落地 `Vec<T>` + `String` + `HashMap<K,V>`**（`Vec<T>`：`new`/`with_capacity` + `push`/`pop`/`get`/`set`/`len`/`cap`/`is_empty` + `v[i]` 索引读写（`check_index` 解 `Vec<T>` 类型参数，步长 8）+ `for x in v` 容器迭代 + 自动扩容 + `contains`/`remove`/`insert`/`clear`/`find`/`sort` 常用操作 + 内建 `alloc_array`/`array_copy`/`array_free`，`vec_test.rs` 6 用例 + `vec_for_test.rs` 6 用例 + `vec_common_ops_test.rs` 6 用例 + `vec_find_sort_test.rs` 6 用例；`String`：UTF-8 字节缓冲（3 槽布局）+ `String::from("字面量")`/`new`/`with_capacity` + `println(String)`（`%.*s`）+ `len`/`cap`/`is_empty`/`get`/`push_byte`/`push_str` + `a + b` 拼接运算符 + 字节级 `s[i]` 索引（步长 1）+ `s1 == s2`/`!=` 内容相等比较（len 短路 + `bytes_eq` 内建 = `memcmp == 0`）+ `s1 < s2`/`>`/`<=`/`>=` 字典序比较（`bytes_cmp` 内建 = `memcmp` 有符号扩展 i64，desugar 为前缀 memcmp + 长度兜底）+ `substring(start, end)` 子串截取（[start, end) 字节区间 + 越界 clamp）+ `find(sub)`/`contains(sub)` 子串查找（朴素滑动窗口，未命中 -1 / 空子串 0）+ 范围切片语法 `s[lo..<hi]`/`s[lo...hi]`/`s[lo<..hi]`（typecheck 层 `check_slice` desugar 为 `String::substring`：`..<` 直通、`...` 闭区间 end+1、`<..` 左开 start+1，仅 String）/ + `to_upper`/`to_lower` 大小写转换（比较链区间 ±32，非 ASCII 不转换）+ `trim` 首尾空白剥离（双扫描 + substring）+ `starts_with`/`ends_with` 前后缀判断（逐字节比较 + 空前缀恒真 + 长于自身恒假）+ `replace` 子串替换（滑动窗口 + 空 old 特判防死循环）+ 顶层 `int_to_string`/`string_to_int` 数值互转（位权除法逐位输出 / 逐字符累加 + 负号 + 遇非数字停止）+ `split` 分割返回 `Vec<String>`（滑动窗口 + 空 sep 特判）/ `repeat` 重复拼接 / `pad_start`/`pad_end` 字节填充 / `strip_prefix`/`strip_suffix` 前后缀剥离返回 `Option<String>` / `truncate` 截断 / + 内建 `alloc_bytes`/`copy_bytes`/`bytes_eq`/`bytes_cmp`/`print_string`/`println_string`，`string_test.rs` 6 用例 + `string_eq_test.rs` 6 用例 + `string_concat_test.rs` 6 用例 + `string_cmp_test.rs` 6 用例 + `string_slice_test.rs` 6 用例 + `string_slice_syntax_test.rs` 6 用例 + `string_case_trim_test.rs` 6 用例 + `string_common_ops_test.rs` 6 用例 + `string_conv_test.rs` 6 用例 + `string_split_pad_test.rs` 6 用例 + `string_strip_trunc_test.rs` 6 用例；`HashMap<K,V>`：开放寻址线性探测 + 墓碑删除 + 翻倍 rehash（负载 1/2）+ 6 槽布局（keys/vals/states/len/used/cap）+ 内建 `hash_value`（整数键 Knuth 乘法散列 / String 键 djb2 内容哈希）+ 构造器特判展开 + `for (k, v) in m` 元组模式迭代 + `keys()`/`values()` 键值集收集 + `clear()` 完全重置（容量不变），`hashmap_test.rs` 6 用例 + `hashmap_for_test.rs` 6 用例 + `hashmap_string_key_test.rs` 7 用例 + `hashmap_keys_clear_test.rs` 6 用例 + `hashmap_len_test.rs` 4 用例，`len`/`is_empty` 已固化（见 5.5/B5））；**补充（见 5.5）**：`Vec<T>` 新增 `first`/`last`/`reverse`/`swap`/`binary_search`（`vec_more_ops_test.rs` 5 用例）；新增 `Duration`/`Instant` 时间模块（libc `clock()` extern 驱动，`time_test.rs` 3 用例）；**io 模块 ✅（B2）**：libc stdio 文件 IO + `read_file`/`write_file`/`append_file`/`read_line`（`io_file_test.rs` 9 用例）；**net 模块 ✅（B3）**：`hostname()` + `htons` + `socketpair_stream`/`fd_at`/`send_all`/`recv_some`/`sockaddr_in4`/`tcp_connect`（依赖本轮位运算全链路，`net_socket_test.rs` 6 用例，见 5.5）；**sync 模块 ✅（B4）**：`Mutex`/`RwLock`（pthread extern + calloc 承载，`sync_test.rs` 6 用例），Condvar/Barrier 待线程创建支持（见 5.5）
- [x] **M2.6 完成（E1）** 交叉编译：`zeta build --target <triple>` 全链路；macOS 双架构已验证（`arm64-apple-macosx` / `x86_64-apple-macosx` 均以本机 clang 直链 SDK 编译运行）；`__zeta_target_os` 平台内建（linux=1/macos=2/windows=3/freebsd=4）驱动 `sockaddr_in4` 双布局（macOS `sin_len` 头 vs Linux 无）；driver 编译目标归一化（`arm64` 前缀统一为 `arm64-apple-macosx`）；Windows/ARM 工具链待环境
- [x] **M2.7 完成（E2）** WASM 目标：`zeta build --target wasm32-wasi` 编译 Zeta 源码为 `.wasm` 并经 wasmtime（WASI preview1）运行验证；`is_wasm_triple`/`assemble_wasm` 支持；`wasm_target_test.rs` 3 用例（triple 判定 + hello world + std 特性数组/String/位运算）全绿，工具缺失时优雅跳过。关键适配（均集中在 driver 的 wasm 链接层，codegen 零改动）：① WASI 入口适配——Zeta 无参 `@main` 重命名为 `__main_argc_argv(i32, i8**)`（wasi-libc `__main_void` 调用约定）；② `-nostdlib` 手动链接 `crt1.o` + `libc.a`（绕开 clang 默认 compiler-rt builtins 缺失：`libclang_rt.builtins-wasm32.a` 不在 Xcode CLT/brew llvm 内，Zeta 的 i64/f64 运算为 wasm 原生指令无需软件例程）；③ wasi-libc 33 多目标布局（`lib/wasm32-wasi/`，旧版 `lib/`）自动探测；④ malloc/memcmp 参数位宽适配——`i64`→`i32`（常量直接降宽、`%` 值前置插 `trunc` 指令），规避 wasm import 签名不匹配（signature_mismatch/malloc_bitcast_invalid trap）。工具链：`brew install lld wasmtime wasi-libc`
- [x] **E3 完成 发布流程**：① `zep publish` 重复版本保护（发布前 `PackageIndex::find` 命中即报错「已存在于注册表，请提升版本号后再发布」，阻止静默覆盖；实测：0.1.0 首次发布成功 → 重复发布报错 → 提升 0.2.0 再发布成功，本地注册表索引累积两版本含 checksum/size）；② `.github/workflows/release.yml` 增加 Windows x86_64（`x86_64-pc-windows-msvc` + `.exe` 后缀 + `shell: bash` 跨平台拷贝），矩阵扩至 macOS ARM64/x86_64 + Linux x86_64 + Windows x86_64 共 4 平台；③ `zeta publish` CLI（`zeta-driver` 依赖 zep lib，`Ctx::new` + `cmd_publish` 委托，`--registry`/`--verbose`；实测：`zeta publish --registry <path>` 发布成功 + 重复版本拦截 exit=1）；④ `CHANGELOG.md` v0.1.0 首个版本记录（语言核心/标准库/工具链/包管理/多目标/CI）
- [x] **M2.8 分配器侧完成** 智能区域（P010：静态大小推断 + PGO 画像/推荐 + EWMA 自适应扩容 + 碎片/事件统计 + 编译器集成报告）；PGO 数据回灌编译流程待完成

### 已完成语言特性（无编号任务）

- [x] **聚合对象语言特性**（enum + match + impl + trait + 泛型单态化 + `struct` 字面量构造 `Point { x, y }` + `&self`/`&mut self` 引用接收者方法 + 方法调用 `Type::method()` + 泛型替换下沉模式绑定 + `Option::None` Infer 占位）
- [x] **for 循环 range 迭代器**（typecheck 层 desugar 为 `loop` + 临时边界变量 + break/continue 正确性；修复 continue 死循环）
- [x] **`for ... in` Vec 容器迭代 + `v[i]` 索引访问**（`check_for` 分派重构：range → `check_for_range`、`Vec<T>` → `check_for_vec`（typecheck 层 desugar 为索引遍历：绑定容器 `__for_v` + 缓存长度 `__for_len`（槽 1）+ 计数器 `__for_i` + `loop { if __for_i >= __for_len { break } let x = __for_v[__for_i]; __for_i += 1; body }`，递增在 continue 回跳前执行无死循环）；`check_index` 解 `Vec<T>` 类型参数并经泛型替换取元素类型（`v[i]` 读 → `HirExpr::Index` 走 IndexSet 赋值，base 为槽 0 data 指针 + 步长 8），`vec_for_test.rs` 6 用例：求和/打印/break+continue/索引读/索引写/空迭代+嵌套）
- [x] **`for (k, v) in m` HashMap 元组模式迭代**（`check_for` 分派新增 HashMap 分支 → `check_for_hashmap`：`AstPattern::Tuple` 二元标识符模式解构 + typecheck 层 desugar 为稀疏索引遍历——绑定容器 `__for_m` + 缓存容量 `__for_cap`（槽 5）+ 计数器 `__for_i` + `loop { if __for_i >= __for_cap { break } if __for_m.states[__for_i] != 1 { __for_i += 1; continue } let k = __for_m.keys[__for_i]; let v = __for_m.vals[__for_i]; __for_i += 1; body }`，`states` 跳槽跳过空槽/墓碑（0/2），K/V 各自经泛型替换取 `field_scalar_of`；`hashmap_for_test.rs` 6 用例：求和/计数/删除后跳槽/空 map/扩容 rehash 后遍历/f64 值类型；遍历顺序与插入无关，测试全部用求和或计数断言）
- [x] **`String` 内容相等比较 `==`/`!=`**（comparison.rs 单比较分支对 `String == String` desugar：绑定操作数到唯一临时变量（防重复求值）+ `s1.len == s2.len && bytes_eq(s1.data, s2.data, s1.len)`，`!=` 取 `Not` 反转；新增内建 `bytes_eq`（typecheck builtin_signature `(Infer, Infer, I64) -> Bool` / LIR BUILTIN_FUNCTIONS 返回 Bool / codegen `declare i32 @memcmp(i8*, i8*, i64)` + `call` + `icmp eq i32, 0` 存 i1）；其他聚合对象（Vec/结构体）的 `==`/`!=` 报 Unsupported（MVP 仅 String）；`string_eq_test.rs` 6 用例：同内容相等/异内容同长度不等/长度不等短路/前缀相同长度不同/中间字节不同/if 分支/复杂表达式操作数）
- [x] **修复：LIR 嵌套 Binary 比较结果错登记为 i64 槽**（既有 bug：`icmp`/`fcmp` 恒产生 i1，但 `lower_operand` 拆平嵌套 Binary 时 `fresh_temp(binary_operand_type)` 对比较运算登记 I64，导致 `let x = a < b` / `println(a == b)` 生成 `store i64 %i1` 类型错误；修复：`binary_operand_type` 保持操作数类型语义，新增 `binary_result_type`（比较/逻辑 → Bool，算术 → 操作数类型），`fresh_temp` 登记结果类型）
- [x] **`HashMap` 字符串键（djb2 内容哈希）**（check_expr.rs builtin 检查处 `hash_value(s)` 参数为 String → 特判 desugar 为纯 Zeta djb2 哈希 HIR 块：绑定 `__s` → 槽 0 data 指针 + 槽 1 len → `__h = 5381` + `loop { if __i >= __len { break } let __b = __data[__i]（字节索引步长 1，LIR 层 zext i64）；__h = __h * 33 + __b; __i += 1 }` → 返回 I64；同内容字符串恒同哈希保证探测链正确，乘法 LLVM mul wrapping 回绕；无新增内建/LIR/codegen 改动，泛型实例化 `HashMap<String,V>` 后 `hash()`/`find()` 键比较 `==` 走内容相等）；`hashmap_string_key_test.rs` 7 用例：insert/get/contains_key/同内容不同对象哈希一致/同键覆盖/remove 墓碑/12 键扩容 rehash/未命中/for 元组迭代）\n- [x] **`HashMap` 键值集/清空：`keys()` + `values()` + `clear()`**（core.zeta 纯 Zeta 泛型 impl 方法；`keys()`/`values()` 稀疏遍历全部槽位收集 `states == 1`（存活）的键/值为 `Vec<K>`/`Vec<V>`——`let mut ks: Vec<K> = Vec::new();` 带泛型参数注解定型（`Vec::new()` Infer 占位需上下文，`grow` 的 `[K; 0]` 先例），遍历顺序与插入无关（开放寻址），同槽位遍历保证 keys 与 values 顺序一致，墓碑/空槽跳过；`clear()` 完全重置——重开三数组（容量不变）替换旧缓冲并 free，`len`/`used` 归零，墓碑与旧数据全部丢弃（与 `remove` 逐键墓碑不同，后续插入即全新探测链，无墓碑堆积拖慢））；`hashmap_keys_clear_test.rs` 6 用例：clear 基本（3 键清空后 len/is_empty/contains_key 全空 + 重新插入可查）/clear 保持容量（cap 8 清空后仍 8 + 复用 4 键不扩容无墓碑残留）/keys 基本（5 键扩容后键集 sort 逐位断言 10→50）/keys String 键（字典序 sort + 与 values 等长同遍历）/values 基本（重复值 + sort 断言 + 与键数一致）/组合（insert + remove 墓碑后 keys/values 只含存活 4 键 + clear 终态键集空））\n- [x] **`Option`/`Result` 补充方法：`expect` + `is_err` + `unwrap_or`**（core.zeta 纯 Zeta 泛型 impl 方法；`Option::expect(msg)`/`Result::expect(msg)` Some/Ok 返回载荷、None/Err 以 `loop {}` 充当崩溃替代（消息参数保留对齐 Rust 语义，MVP 不打印）；`Result::is_err` match 分派 Ok→0/Err→1（与 is_ok 互补）；`Option/Result::unwrap_or(default)` match 分派 Some/Ok 返回载荷、None/Err 返回 default）；为让 expect 的 `msg: String` 参数可引用，`struct String` 定义前移文件顶部（类型符号顺序解析无前向引用）；`option_result_test.rs` 6 用例：Option expect Some 路径（i64/String 双实例化 + 参与算术与比较）/Option unwrap_or 默认值（Some 自身/None 默认/i64 实例化）/Result is_err（Ok→0/Err→1/与 is_ok 互补和恒 1）/Result unwrap_or（Ok 载荷/Err 默认/i64 实例化）/Result expect Ok 路径（载荷 + 算术比较）/组合（带显式类型注解的 Result<i64,i64>/Option<i64> 混合链路 + expect 载荷走 String 方法））\n- [x] **修复：枚举聚合载荷 + 方法返回路径**（`Option<String>`/`Result<String,E>` 等**聚合载荷**实例化下，`unwrap_or` 的 match 分支返回 default（参数/局部 String）曾得到损坏值（垃圾指针数字）；`expect` 的 Some/Ok 分支返回载荷正常、i64 实例化全部正常。根因线索：`field_scalar_of(聚合)=Ptr`——枚举载荷以指针存于 `1 + max_fields` 槽（String 载荷仅 1 字段槽），match 解构恢复聚合的「返回非载荷来源聚合」路径在 codegen 层损坏（R4 排查后经 codegen 改动意外修复）。**已验证修复**：`option_result_test.rs::unwrap_or_string_aggregate` 覆盖 None/Some/Err/Ok 全分支返回 default/载荷 + String 方法链（len/to_upper/==）/拼接/循环，全部正确，`option_result_test.rs` 现 7 用例。另：`Option::Some(10)` 等部分 Infer 类型参数枚举 + 泛型方法实例化需显式类型注解——**已修复**（见下「编译器加固」条目））
- [x] **`String` 拼接 `+` 运算符 + `push_str`**（core.zeta 纯 Zeta 实现 `fn push_str(&mut self, other: String)`：逐字节 `other.data[i]` → `push_byte`（自动扩容迁移），值参数 3 槽拷贝共享缓冲；check_expr.rs `ExprKind::Binary` 分支对 `Add` + 双侧 String 特判 desugar 为 `let __s = a; __s.push_str(b); __s`——`__s` 为 mutable 临时绑定，`push_str` 经 `find_impl_for_method` + `instantiate_impl_method` 实例化注册函数体（mono_instances 防递归重复），返回左侧操作数类型；链式 `a + b + c` 左结合递归展开；`is_string_type` 提升为 `pub(crate)` 供 check_expr 复用）；`string_concat_test.rs` 6 用例：基本拼接/链式拼接/拼接结果字节索引/内容相等比较/push_str 直接调用/多次拼接触发扩容（cap 6→12→24））
- [x] **`String` 字典序比较 `<`/`>`/`<=`/`>=`**（新增内建 `bytes_cmp`（typecheck 签名 `(Infer, Infer, I64) -> I64` / LIR BUILTIN_FUNCTIONS 返回 I64 / codegen `call i32 @memcmp` + `sext i32 to i64` 保留符号）；comparison.rs 单比较分支对 String 排序操作跳过数值类型检查直接 desugar：`string_lt_hir` 绑定 `__a`/`__b` → 槽 1 长度 + min（`if __la >= __lb { __n = __lb }` 控制流赋值）+ `__c = bytes_cmp(__a.data, __b.data, __n)` → `__c < 0 || (__c == 0 && __la < __lb)`；`>` 交换操作数、`<=`/`>=` 取 `Not` 反转）；`string_cmp_test.rs` 6 用例：等长前缀不同定大小/前缀相同长度兜底/完全相等四方向/不等四方向 + == 回归/拼接复杂表达式操作数/if 分支 + 三串取最大）
- [x] **`String` 子串/查找：`substring` + `find` + `contains`**（core.zeta 纯 Zeta 方法；`substring(start, end)` 截取 [start, end) 字节区间——start/end 越界 clamp 到 [0, len]、start >= end 空串，`String::new()` 特判展开 + 逐字节 `push_byte`（自动扩容）拷贝到新缓冲，原字符串不受影响；`find(sub)` 朴素滑动窗口匹配返回首次出现下标（未命中 -1、空子串 0），`result` 变量返回规避 while 块后 `-1` 被解析为减法；`contains(sub)` = `find(sub) >= 0`）；`string_slice_test.rs` 6 用例：基本截取/越界 clamp/find 命中-未命中-空子串/contains 真假 + 自身包含/`find` 下标喂 `substring` 组合提取/拼接 + 子串链式）
- [x] **`String` 范围切片语法 `s[lo..<hi]`/`s[lo...hi]`/`s[lo<..hi]`**（parser 本就支持 `[` 内 range 表达式（RANGE 优先级 infix），`ExprKind::Index` 的 index 为 `ExprKind::Range` 时 typecheck 特判走 `check_slice`（`infer_expr` 对 Range 仅返回 Unit，须在索引推断前分派）；`check_slice` 仅放行 String（`comparison::is_string_type`），数组/Vec 动态切片报 Unsupported，下界/上界要求整数，边界按区间运算符调整后 desugar 为 `HirExpr::Call "String::substring"`（`instantiate_impl_method` 注册函数体，subst 空）——`..<` 左闭右开直通、`...` 双闭 end+1、`<..` 左开右闭 start+1 & end+1；切片结果仍为 String，可链式再切片/拼接/比较，`find` 下标可直接作边界）；`string_slice_syntax_test.rs` 6 用例：三种 range 运算符边界语义/越界 clamp/切片 + == + len/切片链式/拼接 + 切片组合/变量与表达式边界（`find` 下标 + `p + 5`））
- [x] **`String` 大小写/裁剪：`to_upper` + `to_lower` + `trim`**（core.zeta 纯 Zeta 方法；`to_upper`/`to_lower` 逐字节 push_byte 拷贝到新缓冲，比较链区间判断（`97 <= b <= 122` / `65 <= b <= 90`）±32 转换，非 ASCII 多字节不转换；`trim` 剥离首尾空白（空格 32/制表 9/换行 10/回车 13）——正向扫描跳过开头空白找 start、反向扫描跳过结尾空白找 end（嵌套单条件 while + 标志变量，规避 while 头 `&&` 复合条件），再 `substring` 返回新缓冲，全空白串返回空串）；`string_case_trim_test.rs` 6 用例：基本转换（大小写互转 + 再转还原）/混合大小写 + 数字标点原样/空串与单字符/基本裁剪（多空白 + 仅首尾 + 内部空格保留）/全空白 → 空串 + 再裁剪幂等/链式组合（trim + to_upper + substring + find + contains））
- [x] **`String` 常用操作：`starts_with` + `ends_with` + `replace`**（core.zeta 纯 Zeta 方法；`starts_with`/`ends_with` 逐字节比较 + 标志变量，空前缀/空后缀恒 true、目标长于自身恒 false、大小写敏感，result 变量模式返回；`replace(old, new)` 滑动窗口扫描——命中处 `push_str(new)` 追加并跳过 `old.len`（不回溯），否则追加原字节，空 `old` 特判返回自身拷贝防死循环，未命中返回新拷贝原串不受影响）；`string_common_ops_test.rs` 6 用例：前缀基本（命中/不命中/空串/长于自身/全等/大小写敏感）/后缀基本（同构）/前后缀 + 嵌套 if 组合 + substring 提取协议 + find 定位路径/替换基本（单次/多次/未命中原样/内容相等/原串不变量）/替换边界（空 old 拷贝/空 new 删除/old == new/替换串更长）/链式组合（trim + replace + 前后缀 + substring + find + contains））
- [x] **`String` 数值转换：`int_to_string` + `string_to_int`**（core.zeta 顶层自由函数，i64 与 String 互转；`int_to_string(n)` 负数加 '-'（45）前缀 + 0 特判输出 '0'，先算位数 ndigits 再以位权 place（10^(ndigits-1)）从最高位 `(v / place) % 10` 逐位输出，全部 i64 算术；`string_to_int(s)` 逐字符累加 `result * 10 + (c - 48)`，'-' 前缀为负，遇非数字字符停止解析，空串/无数字返回 0）；`string_conv_test.rs` 6 用例：整数转字符串基本（零特判/多位数/拼接结果）/负数（负号前缀 + 数值正确）/转换结果 + String 操作组合（len/starts_with/ends_with/拼接/substring 跳过负号）/字符串转整数基本（前导零/可参与算术）/边界（负数/空串/遇非数字停止/开头非数字/前导空白停止）/往返 + 算术组合（数字→字符串→数字一致/负数往返/乘法除法取模结果转字符串/解析参与比较））
- [x] **`Vec<T>` 常用操作：`contains` + `remove` + `insert` + `clear`**（core.zeta 纯 Zeta 泛型 impl 方法，走 `unify` 统一类型参数 + `instantiate_impl_method` 单态化路径；`contains(x)` O(n) 遍历 `self.data[i] == x`——`==` 在实例化后按具体类型比较（i64 数值相等 / String 内容相等）；`remove(i)` 保存返回值 + 其后元素逐位前移（浅拷贝槽）+ len -= 1，越界（i >= len）以 `loop {}` 充当崩溃替代；`insert(i, x)` 从尾部反向搬移避免正向覆盖 + len += 1 + 容量不足先 grow，i == len 等价 append；`clear()` 仅重置 len（槽区残留被后续 push 覆盖写，无需释放））；`vec_common_ops_test.rs` 6 用例：contains 基本（命中/未命中/空 Vec/多出现）/contains String 元素（内容相等/大小写敏感/子串不等于元素）/remove 基本（删中间/头部/尾部 + 返回值 + 剩余顺序）/remove 序列（连续删除到空 + 复用 push）/insert 基本（中间/头部/尾部插入 + 连续插入扩容 cap 4→8→16）/混合组合（push + insert + remove + contains + clear 全链路最终断言））
- [x] **`Vec<T>` 查找/排序：`find` + `sort`**（core.zeta 纯 Zeta 泛型 impl 方法；`find(x)` result 变量模式返回首个相等元素下标，命中 `i = self.len` 提前终止保住首个下标，未命中 -1（对齐 String::find）；`sort()` 原地选择排序——外循环固定 `min = i`、内循环 `self.data[j] < self.data[min]` 扫最小值，`min != i` 时临时变量三行 swap，O(n²) 非稳定，`<` 在实例化后按具体类型比较（i64 数值 / String 字典序））；`vec_find_sort_test.rs` 6 用例：find 基本（首个命中非末个/头部/尾部/未命中 -1/空 Vec）/find String 元素（内容相等下标/未命中/大小写敏感）/sort 基本（乱序升序 + len 不变 + 首尾）/sort 边界（已排序不动/单元素/空 Vec/重复元素升序）/sort String（字典序 apple→pear）/组合（sort + find 排序后下标 + remove + contains + len））
- [x] **`String` 分割/重复/填充：`split` + `repeat` + `pad_start` + `pad_end`**（core.zeta 纯 Zeta 方法；`split(sep)` 滑动窗口匹配返回 `Vec<String>`——命中处 `substring(start, i)` 推段、跳过 `sep.len` 不回溯，空 sep 特判返回整体自身拷贝防死循环，连续分隔符产生空串段、尾部分隔符尾空段（每段新缓冲）；`repeat(count)` 循环逐字节 push_byte 拷贝（`push_str` 参数为值 String，`&self` 无法直传，改用与 pad 同构的拷贝循环）；`pad_start`/`pad_end(total, pad)` pad 字节（如 48='0'/45='-'）重复填充到总长，total <= len 原样拷贝，空串可填充）；`string_split_pad_test.rs` 6 用例：split 基本（逗号多段/不含分隔符单段/len + get 访问）/split 边界（空 sep 整体拷贝/连续分隔符空串段/尾部分隔符尾空段/空串）/split + 操作组合（to_upper/len/拼接/前后缀）/repeat 基本（3 次/0 次空/1 次自身/内容相等）/pad 基本（左填充 00042/右填充 42000/负号填充 42--/不足原样/空串填充 000）/组合链式（trim + split + to_upper + repeat + pad_start/pad_end 全链路））\n- [x] **`String` 前后缀剥离/截断：`strip_prefix` + `strip_suffix` + `truncate`**（core.zeta 纯 Zeta 方法；`strip_prefix(prefix)`/`strip_suffix(suffix)` 逐字节窗口比较（与 split 同构），命中返回去掉前后缀的剩余部分（新缓冲）、未命中 `Option::None`——`let mut result: Option<String> = Option::None;` 显式注解定型（`Option::None` Infer 占位需上下文），空前缀/空后缀恒命中返回自身拷贝、长于自身恒 None；`truncate(new_len)` 直接委托 `substring(0, new_len)`——负数 clamp 空串、超长整体拷贝、返回新缓冲不影响 self）；`string_strip_trunc_test.rs` 6 用例：strip_prefix 基本（命中剩余 llo/未命中 is_none/完全相等剩余空串）/strip_prefix 边界（空前缀整体拷贝/长于自身/部分前缀/首字节相同后续不同）/strip_suffix 基本（命中剩余 hel/未命中/完全相等空串）/strip_suffix 边界（空后缀整体拷贝/扩展名剥离 filename/长于自身/尾段相近非后缀）/truncate 基本（前 3 字节/0 空串/负数空串/恰好等长/超长整体拷贝/原串不受影响）/组合（trim + strip_prefix + to_upper/pad_end 填充/truncate + repeat/前后各剥一次取 middle））
- [x] **修复：LIR 跨函数返回类型推断——「返回用户函数调用结果」的函数被误判为 i64**（既有 bug：`infer_function_types` 在函数内独立推断，用户 `Call` 的 target 类型不解析（占位 i64），导致 `String::trim` 这类末尾为 `let result = self.substring(...); result` 的函数返回类型被固化 I64，而实际值（substring 返回值 = Ptr i8*）不符，clang 报 `ret i8*` 与函数结果类型 `i64` 不符；修复：提取 `resolve_call_target_types`（lower_function 原 307-341 跨函数解析逻辑），`lower_program` 建立 ret_types 后新增**迭代重算循环**——用 ret_types 解析各函数调用目标 → `infer_return_type` 重算 → 重建 ret_types，迭代至稳定（上限 8 遍）处理链式依赖，第二遍 lower_function 的 return_type 取重算后的全程序表；`string_case_trim_test.rs` 首现该 bug 并随修复全绿）
- [x] **修复：MIR `lower_if` 条件为含控制流的块表达式时「基本块已有终止符」**（既有 bug：`lower_if` 求值 cond 后强制 `self.cur = entry_id` 再发 `CondJump`，当 cond 为含 if 的块（如 `if y > m` 中 `y > m` desugar 出的 min 计算块）时入口块已被内层 if 闭合导致断言崩溃；修复：cond 求值后记录收尾块 `emit_id`，`CondJump` 从收尾块发出——无分支时收尾块 == 入口块行为不变，有分支时正确承载分叉；`lower_while` 本为"发到当前块"模式天然兼容）
- [x] **跨模块路径表达式**（`mod::Enum::Variant` 多段路径 + 模块常量引用 `mod::CONST` + match 模式多段路径 `AstPattern::EnumPath`）
- [x] **数组字面量与索引访问**（AST `ArrayLit` + HIR `Index`/`IndexSet` + MIR/LIR `IndexGet`/`IndexSet` + LLVM GEP 代码生成；数组元素步长 8 字节、字符串字符步长 1 字节；`resolve_ast_type` 保留数组长度 `[T; N]`）
- [x] **标准库 `Vec<T>` 动态数组**（`core.zeta` 纯 Zeta 实现 + 内建 `alloc_array`/`array_copy`/`array_free` 三处登记与 LLVM 映射；`[T; 0]` 长度约定 = 动态数组指针；Vector 3 槽布局 `data(指针)/len/cap`；自动扩容翻倍 + memcpy 迁移；泛型方法实例化 `push`/`pop`/`get`/`set` 全链路可用）
- [x] **标准库 `String` 动态字符串**（`core.zeta` 纯 Zeta 实现，UTF-8 字节缓冲 3 槽布局 `data([u8;0])/len/cap`；构造器 `from`/`new`/`with_capacity` 编译器特判展开（`alloc_bytes` + `copy_bytes` 内建）；`println(String)` 经 `print_string`/`println_string` 内建以 `printf("%.*s", len, data)` 输出；`u8` 字节数组与 String 索引统一步长 1 字节 + load i8/zext、store 前 trunc 的 1 字节读写；修复 `check_if` 非数值分支类型合并 bug）
- [x] **标准库 `HashMap<K,V>` 开放寻址哈希表**（`core.zeta` 纯 Zeta 实现，6 槽布局 `keys([K;0])/vals([V;0])/states([i64;0])/len/used/cap`；`states` 0=空 1=占用 2=墓碑；线性探测 + 墓碑删除不破坏探测链；负载因子 used/cap ≥ 1/2 时翻倍 rehash（新开三数组 + 重哈希，`alloc_array` 需类型注解定型）；`hash_value` 内建三处登记（typecheck `(Infer)->I64` / LIR 返回 I64 / codegen `mul i64, 2654435761` Knuth 乘法散列，MVP 仅支持整数键）；`HashMap::new`（cap 8）/`with_capacity(n)` 编译器特判展开；API：`insert`（同键覆盖）/`get`/`remove`/`contains_key`/`len`/`cap`/`is_empty`；修复块尾 `while ... }` 后直接跟 `-1` 被 parser 解析为减法的隐患（find 改用 `result` 变量返回））

### Milestone 3 — 生态繁荣（Month 8-12）

- [ ] **M3.1** 数据库驱动（SQLite, PostgreSQL, MySQL）
- [ ] **M3.2** HTTP 框架
- [ ] **M3.3** 序列化库（JSON, Protobuf）
- [ ] **M3.4** 嵌入式支持（RTOS, 裸机）
- [ ] **M3.5** GPU 计算后端
- [ ] **M3.6** 官方教程 + 认证考试

---

## 7. 编码规范

### 7.1 Rust 代码（编译器实现）

- 遵循 `rustfmt` 默认配置
- 每个 crate 必须有 `lib.rs` 或 `main.rs`
- 公共 API 必须有文档注释（`///`）
- 测试覆盖率目标：核心模块 ≥ 80%

### 7.2 Zeta 代码（标准库 + 示例）

- 使用 `zeta fmt` 格式化（D2 已实现：`zeta fmt <file.zeta> [-w]`）
- 文件扩展名：`.zeta`
- 模块声明：`module foo { ... }`
- 导入：`import foo::bar;`

### 7.3 提交规范

```
feat(lexer): add support for 'in' keyword
fix(parser): handle edge case in comparison chain
docs(grammar): clarify in-set precedence
test(borrowck): add test for partial move
refactor(mir): simplify CFG construction
```

格式：`<type>(<scope>): <description>`

| Type | 用途 |
|------|------|
| feat | 新功能 |
| fix | Bug 修复 |
| docs | 文档变更 |
| test | 测试相关 |
| refactor | 重构 |
| perf | 性能优化 |
| chore | 构建/工具链 |

---

## 8. 测试策略

### 8.1 单元测试

每个 crate 内 `tests/` 目录或 `#[test]` 标注。

```rust
// crates/zeta-lexer/src/lib.rs
#[test]
fn test_comparison_chain_tokens() {
    let source = "if 0 < x < 10 {}";
    let tokens = lex(source);
    assert_eq!(tokens.len(), 8); // if, 0, <, x, <, 10, {, }
}
```

### 8.2 集成测试

`tests/` 目录下的 `.zeta` 文件，通过 `zeta test` 运行。

```
tests/
├── compile-pass/    ← 应该编译通过的用例（编译到 LLVM IR，不运行）
│   ├── hello.zeta / arith.zeta / struct-trait.zeta
│   ├── modules.zeta / actor.zeta
├── compile-fail/    ← 应该编译失败的用例（`// expect: <片段>` 断言错误消息）
│   ├── type-mismatch.zeta / undefined-var.zeta / unknown-field.zeta
│   └── undefined-fn.zeta / no-method.zeta
└── run-pass/         ← 编译并运行，同名 `.out` 文件精确对比输出
    ├── hello.zeta (+ hello.out)
    ├── arith.zeta (+ arith.out)
    └── actor-ping-pong.zeta (+ actor-ping-pong.out)
```

### 8.3 基准测试

```rust
// crates/zeta-lexer/benches/lexer_bench.rs
use criterion::{black_box, Criterion};

fn bench_large_file(c: &mut Criterion) {
    let source = std::fs::read_to_string("large_input.zeta").unwrap();
    c.bench_function("lex_large_file", |b| {
        b.iter(|| lex(black_box(&source)))
    });
}
```

### 8.4 模糊测试

```rust
// crates/zeta-parser/fuzz/fuzz_target.rs
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(source) = std::str::from_utf8(data) {
        let _ = parse(source);  // 不应该 panic
    }
});
```

---

## 9. CI/CD 流水线

### 9.1 GitHub Actions 配置要点

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]

jobs:
  test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        rust: [stable]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --workspace --all-features
      - run: cargo bench --no-run
      
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - run: cargo clippy --workspace -- -D warnings
      - run: cargo fmt --all -- --check
      
  fuzz:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo install cargo-fuzz
      - run: cargo fuzz run lexer_fuzz -- -max_total_time=60
```

### 9.2 发布流程

1. 更新 `CHANGELOG.md`
2. 打 tag：`git tag v0.x.0`
3. CI 自动构建各平台二进制
4. 上传到 GitHub Releases
5. 更新 `zep` 注册表中的版本信息

---

## 10. 贡献指南

### 10.1 如何贡献

1. Fork 本仓库
2. 创建分支：`git checkout -b feat/your-feature`
3. 编写代码 + 测试
4. 确保 `cargo test` 和 `cargo clippy` 通过
5. 提交 PR，描述变更内容和动机

### 10.2 优先级标签

| 标签 | 含义 |
|------|------|
| `good-first-issue` | 适合新手 |
| `compiler-core` | 编译器核心组件 |
| `std-lib` | 标准库 |
| `tooling` | 工具链 |
| `language-design` | 语言设计讨论 |
| `performance` | 性能相关 |

### 10.3 决策流程

- **小改动**（bug fix、文档）：直接 PR
- **中改动**（新语法特性）：先开 Issue 讨论，再 PR
- **大改动**（内存模型变更、新层级）：RFC 流程

---

## 11. 关键设计决策记录

### ADR-001：比较链方向反转表示区间外

- **决策**：`0 > x > 10` 表示 `x < 0 || x > 10`
- **理由**：数学直觉对称，零学习成本
- **替代方案**：`outside` 等关键词 → 被否决（繁琐）

### ADR-002：区域默认使用 bump allocator + 链表扩容

- **决策**：区域内 bump allocate，溢出时分配新块（翻倍），不拷贝旧数据
- **理由**：扩容 O(1)，总开销远低于逐个 malloc
- **替代方案**：realloc 拷贝 → 被否决（大数据时拷贝开销大）

### ADR-003：Transfer 是编译期操作

- **决策**：`transfer` 不拷贝数据，只转移所有权簿记
- **理由**：零运行时开销，对象仍在原堆位置
- **约束**：不能 transfer 引用，不能部分 transfer

### ADR-004：编译器后端选择 LLVM + Cranelift 双后端

- **决策**：开发期用 Cranelift（编译快），发布用 LLVM（优化强）
- **理由**：兼顾开发体验和生产性能

---

## 12. 参考资源

| 资源 | 链接/说明 |
|------|-----------|
| Rust 参考手册 | 所有权系统设计参考 |
| LLVM 文档 | 后端代码生成 |
| Cranelift 文档 | 快速 JIT 编译 |
| PubGrub 算法论文 | 包管理器依赖解析 |
| Immix GC 论文 | 可选 GC 层级参考 |
| Erlang Actor 模型 | 并发模型设计参考 |

---

## 13. 联系与沟通

| 渠道 | 用途 |
|------|------|
| GitHub Issues | Bug 报告、功能请求 |
| GitHub Discussions | 设计讨论、Q&A |
| Discord | 实时沟通 |
| 邮件列表 | 正式 RFC 讨论 |

---

> **最后更新**：2026-08-20  
> **维护者**：Zeta Language Team  
> **License**：MIT / Apache-2.0
