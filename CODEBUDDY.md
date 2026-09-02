# CODEBUDDY.md — Rlyeh 系统级编程语言

> **项目代号**：Rlyeh  
> **版本**：0.1.0（已收口） / 0.2.0（自举准备，规划中）  
> **状态**：0.1.0 MVP 已收口；0.2.0 自举能力补齐阶段（规划中）  
> **目标平台**：Linux / macOS / Windows / WASM  
> **实现语言**：Rust（自举编译器，bootstrap 阶段用 Rust 实现；0.2.0 起评估 Rlyeh 自举）  
> **自举评估**：[`docs/self-hosting/feasibility.md`](docs/self-hosting/feasibility.md) · 0.2.0 计划：[`docs/development-plan-0.2.0.md`](docs/development-plan-0.2.0.md)

---

## 1. 项目愿景

Rlyeh 是一门面向未来十年基础设施的**系统级编程语言**，设计目标：

| 目标 | 对标 |
|------|------|
| 内存安全、零 GC | Rust |
| 编译速度极快 | Go |
| 并发模型一等公民 | Erlang / Akka |
| 数学式语法直觉 | Python / MATLAB |
| 开发体验友好 | Go / Python |

**一句话定位**：Rlyeh = Rust 的安全性 + Go 的编译速度 + Erlang 的并发模型 + 数学的自然表达。

---

## 2. 项目结构

```
rlyeh-language/
├── CODEBUDDY.md              ← 本文件（项目总纲）
├── CHANGELOG.md              ← 版本变更记录
├── Cargo.toml                ← Rust 工作区配置
├── README.md                 ← 项目介绍
│
├── crates/                   ← 编译器各组件（Rust crate）
│   ├── rlyeh-lexer/           ← 词法分析器
│   ├── rlyeh-parser/          ← 语法分析器
│   ├── rlyeh-ast/             ← 抽象语法树定义
│   ├── rlyeh-hir/             ← 高级中间表示
│   ├── rlyeh-mir/             ← 中级中间表示
│   ├── rlyeh-lir/             ← 低级中间表示
│   ├── rlyeh-typecheck/       ← 类型检查器
│   ├── rlyeh-borrowck/        ← 借用检查器
│   ├── rlyeh-regionck/        ← 区域检查器
│   ├── rlyeh-codegen/         ← 代码生成（LLVM / Cranelift）
│   ├── rlyeh-driver/          ← 编译器驱动（CLI 入口）
│   ├── rlyeh-std/             ← 标准库（Rlyeh 源码）
│   ├── rlyeh-actor-runtime/   ← Actor 运行时
│   ├── rlyeh-region-alloc/    ← 区域分配器（bump + 智能）
│   └── rlyeh-lsp/             ← 语言服务器（LSP over stdio）
│
├── tools/                    ← 工具链
│   ├── rlyeh-fmt/             ← 代码格式化
│   ├── rlyeh-check/           ← Linter
│   ├── rlyeh-doc/             ← 文档生成
│   └── rlyeh-bench/           ← 基准测试框架
│
├── dagon/                      ← 包管理器
│   ├── Cargo.toml
│   └── src/
│
├── docs/                     ← 权威规范 + 开发进度文档（导航见 docs/README.md）
│   ├── README.md             ← 文档导航（入口）
│   ├── guide/                ← 语言教程（每章独立文档，示例均可运行）
│   ├── grammar.md            ← 完整语法规范（EBNF，含规划标注）
│   ├── semantics.md          ← 语义规则（含规划标注）
│   ├── memory-model.md       ← 分层内存管理规范（含规划标注）
│   ├── actor-model.md        ← Actor 并发模型规范（含规划标注）
│   ├── module-system.md      ← 模块系统规范（语法/语义/编译模型，含规划标注）
│   ├── std-lib.md            ← 标准库 API 规范（已实现 / 规划）
│   ├── development-plan.md   ← 开发计划（阶段 A–F 已完成；§6 剩余任务消解 G–L / M–T / U–Z）
│   ├── tasks/                ← 任务文档树（树根 README + milestones + stage-* 索引 + leaf 叶子；任务列表/进度/执行记录以本树为准，见 §7.4）
│   └── design/               ← 早期设计稿归档（映射见 design/README.md）
│
├── examples/                 ← 示例代码
│   ├── hello-world.rl
│   └── arith-print.rl
│
├── tests/                    ← 集成测试（.rl 用例；Rust 集成测试位于各 crate 的 tests/ 目录）
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

```rlyeh
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

```rlyeh
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

### 3.3.1 共享内存并发原语（sync 模块）

```rlyeh
// 互斥锁：带值锁 + 作用域守卫（块尾自动解锁）
let mut m = Mutex::new(0);
{
    let mut g = m.lock_guard();
    let p = (&mut g).get_mut();
    *p = *p + 1;
};

// 读写锁：读-读共享，写-写 / 读-写互斥
let rw = RwLock::new(0);
let r = rw.read_guard();      // 读守卫（&T）
// let w = rw.write_guard();  // 写守卫（&mut T）

// 原子整数（H-M2）：无锁读改写，SeqCst
let a = AtomicI64::new(10);
a.store(20);
println(a.fetch_add(5));                  // 20（返回旧值）→ 当前 25
println(a.compare_exchange(25, 99));      // true
println(a.load_with(sync::Ordering::Acquire));

// 条件变量 / 屏障 / 通道
let cv = Condvar::new();
let bar = Barrier::new(2);
let pair = channel::<i64>();               // ChannelPair { tx, rx }

// 线程：start 接收 move 闭包（跨边界闭包，见 §3.8），join 取返回值
let counter = Arc::new(AtomicI64::new(0));
let c = counter.clone();
let t = Thread::start(move || { c.fetch_add(1); 0 });
match t { Ok(th) => { let _ = th.join(); }, Err(_) => {} }
println(counter.load());                   // 1
```

- **锁**：`Mutex<T>` / `RwLock<T>` 带值锁（`value: T`），`lock_guard` /
  `read_guard` / `write_guard` 返回守卫，**desugar 在所在块尾自动注入 `unlock()`**
  （按方法名特判；`if`/`match` 分支内的提前 `return`/`break` 不注入，可显式调用
  `g.unlock()`）。`*g` 经 Deref/DerefMut 分发（`deref` / `deref_mut` 方法）。
- **通道**：`channel::<T>()` 无界、`bounded_channel::<T>(n)` 有界（背压）；
  `send`/`recv` 阻塞，`try_send`/`try_recv` 非阻塞，`*_result` 返回 `Result`；
  `recv_async()` 返回 future，可 `await`（事件驱动，不阻塞线程）。
- **原子**（H-M2，2026-09-02）：`AtomicI64` + `Ordering`。底层由 driver 注入
  LLVM `atomicrmw` / `cmpxchg`（**无 C 链接符号**，C11 `<stdatomic.h>` 为泛型宏），
  沿用 `__rlyeh_*` 注入机制。RMW 与 CAS 固定 **SeqCst**；`load_with` 支持
  Relaxed/Acquire/SeqCst，`store_with` 支持 Relaxed/Release/SeqCst。
- **已知限制**：仅 `AtomicI64`（无 `AtomicBool`/`AtomicUsize`/`AtomicPtr`）；
  无 `fetch_update` / `fetch_max` / `fetch_min` / `compare_exchange_weak`；
  `Ordering::AcqRel` 在 load/store 分派中归入 SeqCst；锁与原子对象无析构
  （缓冲由 OS 在进程退出时回收）。

### 3.4 区域系统

```rlyeh
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

// 精确预分配 / 显式 bump 策略（L3 ✅：region 指令接线 rlyeh-region-alloc C ABI 运行时）
region 's with_size (4096) {
    let buf = BigStruct::new() in 's;
}
region 't strategy (bump) {
    let p = Point { x: 1, y: 2 } in 't;
}
```

### 3.5 聚合对象与泛型

```rlyeh
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

### 3.5.1 元组（值构造 + 解构 + 多返回值）

```rlyeh
// 元组值构造 + 按位置字段访问 `t.f0` / `t.f1` / ...
let t = (10, 20, 30);
println(t.f0);                    // 10
let hetero = (7, "rlyeh");        // 异构元组（元素类型可不同）

// 解构绑定（M2，SH-P0-5）
let (a, b) = (1, 2);
let (x, _, z) = (10, 20, 30);     // `_` 跳过该位置（仍占用下标）
let mut (p, q) = (1, 2);          // 整体可变
p = 10;

// 多返回值 + 解构接收（M3）
fn split_name(s: String) -> (String, i64) { (s, s.len()) }
let (name, len) = split_name(String::from("rlyeh"));   // len = 5
```

语义：元组为 N 槽聚合，元素按位置命名 `f0..fN-1`；解构 desugar 为
「临时变量承载元组值（init 只求值一次）+ 各元素按位置 `FieldGet` 绑定」。

> 已知限制：解构的**元素模式仅支持标识符与 `_`**——无嵌套解构
> （`let ((a, b), c) = ..` 报错）、无 `(mut a, b)`（parser 不接受元组模式内的
> `mut`）、无 `..` 剩余模式；结构体 / 枚举解构模式仍不支持。

### 3.5.2 `if let` / `while let` 模式控制流（SH-P0-6 ✅，2026-09-02）

```rlyeh
// 匹配成功则绑定，失败走 else（else 可省，此时不匹配即跳过）
if let Option::Some(v) = opt_value() {
    println(v);
} else if let Option::Some(w) = other() {   // else if let 链
    println(w);
} else {
    println(-1);
}

// 作表达式使用（desugar 结果即 match，天然是表达式）
let v = if let Option::Some(x) = o { x } else { 0 };

// 每轮重新求值，模式不再匹配即跳出；体内 break / continue 落在循环上
while let Option::Some(got) = rx.recv() {
    println(got);
}
```

语义：**纯语法糖，parser 层展开、零新增 IR 节点**——
`if let Pat = e { A } else { B }` ⟶ `match e { Pat => { A }, _ => { B } }`；
`while let Pat = e { A }` ⟶ `loop { match e { Pat => { A }, _ => break } }`。
缺 `else` 时兜底空块（求值 `()`），保证 `match` 始终穷尽；`else if` /
`else if let` 链由 `if` 解析递归处理。落点仅
`crates/rlyeh-parser/src/expr/control.rs`，typecheck / codegen 无改动。

> 已知限制：模式能力继承 `match` 臂——元组 / 结构体模式在 match 位置尚不支持
> （`if let (a, b) = t` 报 `unsupported syntax: 元组 / 结构体模式在 MVP 阶段`，
> 可解构绑定 `let (a, b) = t;` 走另一路径不受此限）；
> 无 let 链 `if let a = .. && let b = ..`。

### 3.5.3 `match` 守卫 / 范围模式 / 或模式（SH-P0-7 ✅，2026-09-02）

```rlyeh
// 守卫 `pat if cond`（绑定变量在守卫内可见）
match o {
    Option::Some(v) if v > 10 => 1,
    Option::Some(v) if v > 5  => 2,
    Option::Some(_)           => 3,
    Option::None              => 4,
}

// 范围模式 `lo..<hi`（上开） / `lo...hi`（双闭） / `lo<..hi`（下开）
fn classify(n: i64) -> i64 {
    match n {
        0        => 0,
        1...9    => 1,   // [1, 9]
        10..<20  => 2,   // [10, 20)
        20<..30  => 3,   // (20, 30]
        _        => -1,
    }
}

// 或模式 `A | B`（解析器字符分类的典型形态）
match c {
    'a'...'z' | 'A'...'Z' if upper => 1,   // 与守卫组合
    'a'...'z' | 'A'...'Z'          => 2,
    '0'...'9'                      => 3,
    _                              => 0,
}

// 或模式各备选可绑定同名同序变量
match s {
    Shape::Circle(r) | Shape::Square(r) => r,
    _ => -1,
}
```

语义要点：
- **守卫**在绑定**之后**求值——条件构造为 `if <模式条件> { <绑定>; <守卫> } else { false }`。
  不可直接 And 合并：MIR 的 `&&` 是**非短路**的（`lower_expr` 对 `Binary` 两侧无条件
  求值），会让绑定被无条件执行；借 HIR `If` 的真实 CFG 分叉才获得短路语义。
- **范围模式**复用比较链的 `check_comparison` / `compare_hir`，与 `x in lo..<hi` 同语义。
- **或模式**的 `|` 仅在 match 臂与 `if let` / `while let` 的模式位置生效——闭包参数
  列表的 `|` 是分隔符，模式解析吞 `|` 会误食参数列表结束符。

> 已知限制：守卫**仅对可反驳模式可用**（标识符 / `_` 兜底模式报
> `对兜底模式使用守卫条件`，与 Rust 的 `x if cond` 不同）；或模式不支持不可反驳
> 备选（`_ | 1` 报错）且各备选须绑定同名同序变量；范围模式仅数值与字符、
> 边界须为字面量。

### 3.5.4 `Drop` trait / 析构 / RAII（SH-P0-8 ✅ Q1–Q3，2026-09-02）

```rlyeh
struct Resource { id: i64 }

impl Drop for Resource {
    fn drop(&mut self) { println(self.id); }   // 离开作用域时自动调用
}

// 逆字段序递归（drop glue）：自身无 Drop 的 struct 析构其拥有字段
struct Outer { a: Resource, b: Resource }

fn main() {
    {
        let a = Resource { id: 1 };
        let b = Resource { id: 2 };
        println(100);
    }                          // 逆声明序析构：先 b(2)，再 a(1)

    let v = { let r = Resource { id: 3 }; r.id };
    println(v);                // 块值先求（3），再析构 r
}
```

语义要点：
- `Drop` 是**编译器内置 trait**（与 `Any` 同构），无需显式 `trait Drop` 声明；
  `impl Drop for T` 即注册析构，`&mut self` 无需 `let mut` 绑定。
- 析构在**块尾**按**逆声明序**插入，仅对拥有所有权的绑定（引用 `&T` 跳过）；
  嵌套块按词法作用域各自在块尾析构。
- **块值先求后析构**：有值块先把尾表达式存入临时，析构后再以该临时作为块结果。
- **字段级 drop glue**：先 `T::drop()`，再按逆字段序递归析构拥有字段（仅具名
  struct 参与，深度上限 4 防 `A{b:B}` / `B{a:A}` 自引用无限展开）。
- 析构调用经常规方法解析（`x.drop()`），**零新增 IR 节点**；**无 `Drop` 实现的
  类型不产生任何语句**，故存量代码零影响。

> 已知限制：不跟踪 move（被 `return` 移出或转移给其他值的变量仍会被析构）；
> 函数**形参不析构**（位于外层 fn 作用域）；`return` / `break` 提前退出路径不
> 注入（与既有 `guard.rs` 同一约束）；**Q-M4 智能指针未接入**——`Box` / `Rc` /
> `Arc` 尚无 `Drop`，故 `MutexGuard` 自动解锁仍走 `lock_guard` 方法名特判的
> 既有注入路径。

### 3.5.5 泛型 trait / impl 与约束（SH-P1-1 ✅ A4，2026-09-02）

```rlyeh
// 泛型 trait 声明 + 泛型 impl（A1 / A2，既有能力）
trait Wrap<T> { fn wrap(&self, v: T) -> i64; }
impl<T> Wrap<T> for Pair<T> { fn wrap(&self, v: T) -> i64 { .. } }

// 含 `Self` 返回（A3，既有能力）
trait From2<T> { fn from2(v: T) -> Self; }

// 函数 / 方法级 `where` 子句（A4-①，本项新增）
fn loud<T>(x: T) -> i64 where T: Speak { x.speak() }
fn both<T>(x: T) -> i64 where T: Speak + Named { x.speak() + x.name_id() }
fn mixed<T: Speak, U>(x: T, y: U) -> i64 where U: Named { x.speak() + y.name_id() }

// impl 块级约束（内联与 `where` 两种写法，A4-② 起强制校验）
impl<T> Wrap for Pair<T> where T: Speak { fn wrap(&self) -> i64 { self.a.speak() } }
```

要点：
- `where` 子句可用于**函数、impl 块内方法、trait 抽象方法、impl 块**，与内联
  bound（`<T: B>`）等价且可混用；约束按参数名合并，诊断统一为
  ``type `X` does not implement trait `B` (bound on generic parameter `T`)``。
- impl 级约束在**调用点实例化方法体之前**校验（此前只记录不校验，违反时在方法体
  内部报出误导性的 `i64::speak not found`）。

> **A2 两处限制已修复（2026-09-02）**：
> - ① 同一类型的同一泛型 trait 的**多 impl 可按 trait 类型实参 / 实参类型选择**——
>   `check_method_call` 改为收集全部候选 impl（`find_impl_candidates` /
>   `find_trait_method_candidates`），按「代入 `trait_type_args` 后的方法签名与
>   实参类型兼容」选取首个匹配者。
> - ② impl 的类型参数**可由实参反推**——选取候选时把 impl / 方法级未定泛型
>   （`cand.type_params` ∪ 方法泛型）由对应实参 `unify` 绑定，故 `impl<T> Wrap<T>
>   for W`（`W` 非泛型）的 `T` 可由实参推导。
> - 附带修复：同 trait 多 impl 的**单态化缓存碰撞**——`instantiate_impl_method`
>   的 mono 键 / 后缀此前只含 `impl_def.type_params`（此处为空），未含
>   `trait_type_args`，导致 `impl Wrap<i64> for W` 与 `impl Wrap<bool> for W` 产生
>   相同 mono 键 → 缓存命中复用首个实例，方法体 / 接收者错配。现 mono 键并入
>   `trait_type_args`，该回归用例见 `tests/run-pass/generic_impl_multi.rl`。

### 3.6 模块系统

```rlyeh
module math {
    pub const PI: f64 = 3.14159;
    pub fn square(x: i64) -> i64 { x * x }
}
import math::PI;                 // 别名导入
import math::square as sq;

// 多文件模块：module foo; → foo.rl / foo/module.rl（扁平名字空间，路径即「模块名::」）
// 跨模块路径：模块名::Enum::Variant / 模块名::CONST
```

### 3.7 数组与索引访问

```rlyeh
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

// 切片引用 `&[T]` / `&mut [T]`（S1/S2/S3 ✅，2026-08-30）：零拷贝**胖指针**
// （2 槽 {data 指针, 长度}，布局同 `&str` 的 StrFat）；`[T]` 为 DST，不能独立存储。
fn sub_len(xs: &[i64]) -> i64 { let sub = xs[1..<3]; sub.len() }
let xs = [10, 20, 30, 40, 50];
sub_len(&xs);                  // 2：&[i64; 5] 经 unsize coercion → &[i64]，再切片长 2
// 切片方法：.len() .first() .last() .iter() .as_ptr() .as_mut_ptr()
let bp = v.as_slice();         // Vec<u8> → &[u8]（紧凑字节，步长 1）
let bm = v.as_mut_slice();     // → &mut [u8]，写回原缓冲（零拷贝）
// 注：数组 `arr` 的范围切片按值拷贝返回 Vec；切片接收者的范围切片返回零拷贝子区间。

// 类型联合 `A | B`（U1/U2 ✅，2026-08-30）：成员须**两两互不相交**，
// 运行时为匿名 enum（槽 0 = tag、槽 1 = payload），复用现有 enum codegen。
let x: i64 | String = 5;            // 成员值直接构造联合（协变）
match x {
    i64 => println(i64),            // 类型臂：payload 绑定到类型名同名变量
    String => println(String.len()),
}
// 非法（报 UnionMembersNotDisjoint）：i64 | i64、&i64 | &mut i64、i64 | isize
// 优先级：`&T | &mut U` = (&T) | (&mut U)；闭包注解 `|x: i64| ..` 的 `|` 非联合运算符。
// 未收窄的联合禁止直接运算 / 方法调用（`compatible_with` 单向：成员 → 联合）。

// 枚举显式判别式（U3，2026-08-30）：判别值即该变体的 tag，构造与 match 均复用。
enum Code { Ok = 200, NotFound = 404, Error = 500 }
```

### 3.8 函数一等值（函数指针）

```rlyeh
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
let name = String::from("rlyeh");
let r6 = (|s| s + name)(String::from("hi "));  // hi rlyeh：捕获 name

> H2 无捕获闭包约束：参数无类型注解，需 fn 类型上下文驱动推断（fn 形参实参 / `let f: fn(..) = |..| ..` 注解）；闭包体仅可引用参数与字面量；参数模式仅支持简单标识符与 `_`；返回闭包的函数已支持（H5 补全：`fn make() -> fn(..) { |x| .. }` 尾闭包按 H2 签名检查生成函数指针返回）。
> H3 捕获闭包（IIFE MVP）：`(|x| body)(args)` 立即调用时 body 中引用的外层变量按值捕获，desugar 为匿名函数 `__closure_N(cap..., x)`（捕获变量作前置参数、参数名保留原名）+ 普通函数调用，零新增 IR 节点；字符串字面量实参自动升级为 String 语义；`move` 关键字 MVP 忽略（所有权宽松）。
> H5 闭包值对象（MVP）：`let f = |x: i64| body;` 绑定后 `f(..)` 反复调用——按值捕获 desugar 为捕获聚合对象（每捕获一槽 `Alloc` + `FieldSet`）+ 调用点 `__closure_N(FieldGet(f, i)..., 实参...)` 展开，零新增 IR 节点；参数类型确定规则：有注解用注解类型，无注解由首次调用点实参推断（延迟固化，`let f = |x| x + 1; f(41);`，半注解亦可用），从未调用则不检查闭包体（惰性）。H5 补全：**返回闭包的函数**（`fn make() -> fn(i64) -> i64 { |x| x + 1 }` 尾闭包按 H2 签名检查；`let f = |x: i64| x + 1; f` 作返回值时无捕获闭包值降级为 fn 指针；`let f = |x| x * 2; f` 未固化闭包按返回签名固化）；**闭包值作 fn 实参**（无捕获闭包值 → fn 形参降级/固化为函数指针 `apply(f, 41)`；有捕获闭包值经 fn 签名传递报 Unsupported/类型不匹配）。限制：仅按值捕获；捕获闭包值不跨函数边界（不能作 fn 实参/返回值）；`move` 忽略、按引用捕获规划中。

```rlyeh
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

> H4 `dyn Trait` 约束：trait 与 impl 均须非泛型；vtable 的 drop/size/align 槽 MVP 置 0（显式释放语义与 `Box`/`Rc` 一致）。
> 含 `Self` 签名的方法：dyn 变量绑定源具体类型已知时（如 `let d: dyn T = &obj;`）**可调用**——devirtualize 把 `Self` 替换为具体类型后静态分派，形参 `&Self` → `&P`、按值返回 `Self` → `P`（见 `tests/run-pass/dyn_self_return.rl`）；**完全擦除**具体类型的 `dyn Trait`（如作函数参数传入）仍按 object-unsafe 拒绝（与 Rust 一致）。
> 注：被 vtable 取址的 trait 方法**不参与**「标量聚合按值返回」优化（经函数指针间接调用须保持稳定的 `i8*` 返回 ABI），按值返回聚合时退化为堆分配 + `i8*` 返回——与未被取址的普通函数（如 `make_pair` 的 `{i64,i64}` 寄存器返回）不同。

### 3.8.1 `dyn Any` 类型擦除与 downcast（G ✅，2026-09-01）

```rlyeh
struct Point { x: i64, y: i64 }
struct Msg { tag: i64 }

let p = Point { x: 7, y: 8 };
let a: dyn Any = &p;                        // 装箱：擦除为 dyn Any

let id = any_type_id(a);                    // i64：运行时类型标识
let rp = any_downcast_ref::<Point>(a);      // Option<&Point>
match rp {
    Option::Some(pt) => println(pt.x),      // 7：类型匹配，还原为 &Point
    Option::None => println(-1),
}
let wrong = any_downcast_ref::<Msg>(a);     // 类型不符 → None（安全，非未检查转换）
```

语义：`&T → dyn Any` 为编译器内置的类型擦除——vtable 仅 3 元槽，**槽 0 存具体类型的
type_id**（`type_id_of` = 类型规范字符串的 FNV-1a 64 位散列，编译期确定、全程序稳定）。
`any_type_id(x)` 读回该标识；`any_downcast_ref::<T>(x)` 展开为
`if type_id(x) == type_id_of(T) { Some(data_ptr as &T) } else { None }`——
**类型标识相等才产出具 `&T` 的 `Some`**，错误类型向下转换得到 `None`。

门禁：非 `dyn Any` 实参、缺 turbofish 类型参数均显式报错。
限制：无 `Box<dyn Any>` / 多 trait 约束（`+ Send`、auto trait）；无按值 `downcast`（仅有引用形式 `downcast_ref`）。

---

### 3.9 MVP 已知限制（规划中特性）

以下语法可解析但 MVP **未实现**（typecheck 显式报 Unsupported），详见 [`docs/guide/13-references-limits.md`](docs/guide/13-references-limits.md) §13：

| 特性 | 说明 |
|------|------|
| 宏调用 | ✅ 已实现：`macro_rules!` 声明式宏（`$x:expr`/`$x:ident`/`$x:ty`/`$x:tt` 元变量 + `$(`...`)` 重复 `*`/`+`/`?`，parse 期递归展开为 AST）——`$x:expr` 捕获**完整表达式 token 序列**（token 级优先级爬升：前缀一元 `-`/`!`/`not`/`&`/`*`、二元中缀、后缀调用/索引/成员/`?`/`as` 转换、嵌套宏调用 `m!(x)`，按原样展开优先级由调用方负责）+ 内置格式化宏 `println!`/`print!`/`format!`/`dbg!` + `eprintln!`/`eprint!`（stderr，N4 ✅；`{}` 值占位、`{:?}` 同构、`{{`/`}}` 转义、多参数可变长度；typecheck desugar 为 String 拼接 + 内建打印，`eprint*` 经 POSIX `dprintf(2, ...)` 直写 stderr）+ 集合宏 `arr!`/`vec!`/`map!`（I3 ✅，§3.9：`arr!` → 数组字面量；`vec!`/`map!` → 块表达式 `Vec::with_capacity(n)` + 逐元素 `push`/`insert`，空集合 → `new()`，元素经子 Parser 解析支持嵌套宏调用与完整表达式）；`r#"..."#` 带哈希原始字符串已实现（lexer：任意 `#` 定界、无转义，与 `r#ident` 区分）；`!` 保留 `not` 一元运算符语义。限制：无卫生宏（hygiene） |
| 引用类型 | `&x`/`&mut x` 表达式、`&T`/`&mut T` 参数与返回、解引用 `*` 已实现（G1 ✅，标量存 `i8*` 槽、聚合拷贝指针、字段/方法自动剥引用层）；裸指针（G3 ✅）：`*const T`/`*mut T` 类型 + `*p` 读写 + `&T`↔`*const T` 互视（宽松）+ `*mut` 降级 `*const`；生命周期标注（G4 ✅ MVP 语法接受）：`<'a>` 与 `&'a T` 解析后丢弃（宽松检查）；`&str` 只读借用视图（G2 ✅：`String::as_str()` + `&str` 参数/返回/索引 + `String::from(&str)` 深拷贝）；**`str` 值一等类型**（字符串字面量 / 绑定字面量的变量：方法调用 / `+` 拼接 / 内容比较自动升级为 String 对象，编译期长度展开）；**String 形参位置的字面量实参自动升级**（`m.push_str("!")`/`m.contains("z")`/`map.insert("k", 1)`/`f("hi")` 直接可用——覆盖实例 / 静态方法、普通函数、函数指针、泛型调用与 `dyn Trait` 方法，与 IIFE / 闭包值实参升级语义一致）；**`ref` / `ref mut` 模式已实现**（G1 收尾：`match` 臂与 `let ref x = e;` 绑定变量为对匹配值的引用而非值拷贝，枚举子模式 / struct 字段 / 解引用写均可用；MVP 注意——`match` 先拷贝匹配值，`ref` 绑定指向拷贝，`ref mut` 写与原变量无关）；**严格借用检查已实现**（G1 收尾，Rust E0502/E0499/E0596/E0597 对应）：NLL 近似的借用排他性——`&mut` 与任何活跃借用互斥、多个 `&mut` 互斥、活跃可变借用期间写入被借用变量报 `BorrowConflict`；`&mut` 要求 `let mut` 绑定（`BorrowMutImmutable`）；局部引用逃逸函数（尾表达式 / `return` 返回 `&x` 或绑定引用变量）报 `DanglingReference`（参数来源引用允许返回）；语义有意宽松——读取被借用变量与经 `*p` 写入允许（裸指针别名合法），共享借用（多个 `&`）可共存，仅直接赋值被借用变量触发冲突；`print`/`println` 参数为引用时自动剥层打印解引用值（`println(r)` ≡ `println(*r)`）；生命周期标注（`'a`）仍为语法接受宽松检查，严格生命周期验证规划中 |
| 闭包 | ✅ H2 无捕获闭包已实现（§3.8）：`\|x, y\| expr` desugar 为匿名函数（`__closure_N`）+ 函数指针（typecheck `check_closure_expected` 按预期 fn 签名检查闭包体，注入全局 HirItem，零运行时开销）；需 fn 类型上下文（fn 形参实参 / fn 注解绑定）。✅ H3 捕获闭包（IIFE MVP）：`(\|x\| body)(args)` 立即调用按值捕获（迭代收集捕获变量 + 类型快照，desugar 为匿名函数 + 捕获变量前置调用，零新增 IR 节点）。✅ H5 闭包值对象（MVP，含补全）：`let f = \|x: i64\| ..; f(..)` 绑定后反复调用（按值捕获 desugar 为捕获聚合对象 + 调用点字段读取展开，零新增 IR 节点，仅局部变量环境）；参数类型规则——有注解用注解、无注解由首次调用点实参推断（延迟固化，半注解亦可用）；**返回闭包的函数已实现**（`fn make() -> fn(..) { \|x\| .. }` 尾闭包按 H2 签名检查；无捕获/未固化闭包值作返回值经 fn 指针降级/签名固化）；**无捕获闭包值可作 fn 实参**（降级/固化为函数指针）；限制：捕获闭包值不跨函数边界（作 fn 实参/返回值报错）、`move` 忽略、按引用捕获规划中 |
| 函数指针 | ✅ 已实现（H1）：`fn(T) -> R` 类型 + `let f = add` 函数值绑定 + `f(args)` 间接调用（typecheck `Type::Fn` → HIR/MIR/LIR `CallIndirect` → LLVM `i8*` 槽 + 按签名 `bitcast` + 间接 `call`）；函数值可作实参、返回值、重新绑定、类型注解；`&T` 已实现见上 |
| 运算符 | `?` 错误传播已实现（K1 ✅）：`expr?` 在 Option/Result 上下文 desugar 为 `match { Some(__v) => __v, None => return Option::None }`（Result：`Err(__e) => return Result::Err(__e)`），复用 check_match 的 if-else 链 + tag 比较，零新增 HIR 节点；支持表达式中间嵌套 `?`（如 `Some(a? + b?)`）；裸无参变体值表达式（`return None;`）可用；`?` 用于非 Option/Result 类型报 Unsupported。**`as` 数值转换已实现（U6 ✅，§3.2）**：`expr as T` 数值→数值——`f64↔i64` `fptosi`/`sitofp`（向零截断）、整数截断/扩展 `trunc`/`sext`/`zext`（`300 as i8`=44）、整↔bool `icmp ne 0`/`zext`（`5 as bool`=true）、整↔char（`'a' as i64`=97，char 已拓宽为 32 位 Unicode 码点，codegen i32，可表达任意 Unicode 字符）、同类型零指令；范围：≤64 位整族 + 浮点 + bool + char（32 位 Unicode 码点），i128/u128 与指针/引用/聚合转换保持擦除。`dyn Trait` ✅ 已实现（H4，§3.8）：trait 对象（`dyn Trait` 类型 + `&T` 强制转换 → vtable + 2 槽胖指针 + 方法调用 vtable 间接分派），MVP 限制：非泛型 trait/impl、含 `Self` 签名方法不可经 dyn 调用 |
| 所有权层级 | K2 `Box<T>` + K3 `Rc<T>`/`Arc<T>` + K4 `Gc<T>` ✅（§3.11）：堆分配 + `*` 解引用 + 字段/方法/索引自动剥层 + 引用计数（clone/强弱计数/弱引用/`try_unwrap`）+ 可选 GC（`gc_region` 块 + 逃逸 root + 保守标记-清除，`rlyeh-gc-runtime`） |
| 并发 | `fmt` 模块规划；actor 的 `async` 方法 + `.await`/`send` 已实现（§3.3）；**actor 交叉编译 / WASM 支持（L4 ✅，guide/11-targets-toolchain.md §11.3：`wasm32-wasip1` 目标下 driver 注入静态 `rlyeh_actor_resolve` 符号表替代 dlsym + WASI 单线程同步运行时，ask/send/FIFO/受监督崩溃重启协议与 native 一致；需先 `cargo build --target wasm32-wasip1 -p rlyeh-actor-runtime`）**；普通函数 `async fn`/`.await` 已支持（**S1c ✅，guide/09-actors.md §9.3 + std-lib.md §10.3：`async fn` desugar 为 Future 结构体 + poll 状态机 + 构造器（`rlyeh-desugar`），`expr.await` 经状态机轮询子 future，支持 `Poll::Pending` 挂起/恢复与跨 await 变量提升，`block_on` 轮询驱动；MVP 限制：参数 `i64`、返回 `i64`/`()`；**W1 ✅（2026-08-25）Future 泛型化——`type Output` 关联类型 + `cx: &mut Context` 参数（`fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output>`），desugar 与手写 `impl Future` 经 `&mut *cx` 透传，`Context` 携带 `deadline` 槽（W3）供事件驱动唤醒**；**W2 ✅（2026-08-25）控制流图展开——`if`/`while`/`for`/`loop`/`match` 子块内 await 递归展开为扁平段（段携带显式后继回跳），表达式中间嵌套 await 经临时变量 `__w_N` 提取，跨 await 变量类型扩展至 f64/bool/char/String；控制流汇合经占位段 `ph` 保证初始 `state=0` 指向首执行段，poll 末尾兜底 `Poll::Pending` 由 `block_on` 循环推进**；**W4 ✅（2026-08-25）`future::join_all` + `timeout` Future 版 + `TimeoutError` + `F::Output` 投影——`join_all<F: Future>(Vec<F>) -> Vec<F::Output>` 并发轮询直至全部 Ready（结果经 `HashMap<i64, F::Output>` 按序收集，支持 String/bool 等任意 Output），`timeout<F: Future>(d, &mut F) -> Result<F::Output, TimeoutError>`；**`F::Output` 关联类型投影落地**（`Type::AssocProjection` + `resolve_ast_type` 识别 `F::Output` + 实例化求值查 trait impl 关联类型）；泛型 bound 裸名解析修复（`F: Future` 可解析 std import 提升的 trait，新增 `resolve_trait_def_name`）**；**W3 ✅ 第一步 定时器事件驱动（2026-08-25）——`Context` 升级携带 `deadline` 槽，future `Pending` 时经 `&mut *cx` 写入下次唤醒截止；`block_on`/`timeout` 据此 `thread::sleep` 到该时刻（timeout 取「future 唤醒 / 超时截止」更早者）再轮询，进程真正休眠非忙等（deadline=0 退回忙等向后兼容）；`future::sleep(d)` 定时器 future（`Sleep`），经完整路径 `future::sleep` 调用，await 走带注解变量（`let s: future::Sleep = future::sleep(d); s.await`）；实证 sleep(20ms) 墙钟 23ms / CPU 37us**；**W3 ✅ 第二步 fd 事件唤醒（2026-08-25）——`Context` 再携带 `fd`/`interest` 槽，future `Pending` 时可请求监听某 fd 读/写就绪；`block_on` 在 `Pending` 且 `fd > 0` 时构造 `Poller`（poll(2)）注册并 `poll` 等待就绪（进程休眠非忙等）；`future::wait_fd(fd, interest)` 落地（`WaitFd` future，poll(0) 检查就绪，未就绪写 `cx.fd` 并 Pending）；实证 TcpListener + 线程延迟 connect：墙钟 35ms / CPU 0.3ms；为 W5（recv_async/HTTP async 真异步）提供底座**；**W5 🔧 `recv_async` 真异步（2026-08-25）——`Channel` 携带 socketpair 唤醒 fd（`send`/`close` 写 `wake_w` 触发 `wake_r` 读就绪），`RecvAsync` future（持 `Rc<Channel>`）poll 经 `try_recv` 非阻塞取消息、空则写 `cx.fd = wake_r` 挂起（W3 事件驱动）、close 且空返回 -1；用法 `let r: sync::RecvAsync = rx.recv_async(); r.await`（或 `block_on(&mut r)`）；**W5 ✅ 完整交付——`recv_async` + `get_async` + `post_async` 全部真异步：`GetAsync` future（connect/写同步 + 非阻塞读响应 wait_fd 挂起，`extract_body` 解析，POST 带 body + Content-Length），`block_on` 泛型化返回 `F::Output`（支持 Response）**；`core.rl` 重排 `module future` 到 `module net`/`sync` 前使能解析跨模块 future 符号**）；`json` 序列化已实现（L2 ✅，§3.12：`json::stringify`/`json::parse::<T>`，turbofish 泛型实参；标量/数组/struct/Vec/`HashMap` 序列化 + `i64`/`bool`/`String`/`HashMap` 反序列化；`Serialize`/`Deserialize` trait 与 `#[derive]` 宏规划） |
| 迭代器 | ✅ J1–J3 已实现（§3.10）：`for x in arr` 数组迭代（索引遍历，长度编译期已知）+ 自定义迭代器接入 `for`（存在 `next() -> Option<T>` 方法，inherent/trait impl，desugar 为 `loop { match it.next() { Some(x) => body, None => break } }`）+ 适配器 `map`/`filter`/`fold`/`collect`/`take`/`skip`（数组/Vec/迭代器接收者，经 H2 闭包，返回 `Vec<T>` 可链式）；数值区间、`for x in vec`/`for (k, v) in map` 亦可用；**`Vec::iter`/`iter_mut` 瘦指针迭代器（V1 ✅：`Iter<T>`/`IterMut<T>` 裸指针 + 剩余长度，零分配零拷贝，`next()` 值拷贝读取 / `write()` 写回真实原槽，接入 for 与适配器；`Vec::iter_ref` 引用元素迭代器（V1 ✅ 补完，2026-08-29：`IterRef<T>` 零拷贝返回 `Option<&T>` 元素引用，等价 `Iter<'_, T>`，支持 `*r = x` 原地写回）；`HashMap::iter_pairs` 经 `KVRef<K,V>` 零拷贝 KV 引用迭代（V1 ✅，2026-08-29，等价 `(&K,&V)`）；数组/`Vec` 适配器（map/filter/...）因数组非命名类型保留内建 desugar（返回 `Vec`），记为已知语言限制）**；`Iterator` trait 定义（std-lib §2.3）为规划 API（适配器为编译器内建）；**String 字符/行迭代器（V2 ✅，2026-08-29）：`String::chars()` 返回 `Chars` 码点迭代器（`next() -> Option<char>`，UTF-8 解码，元素为 32 位码点）、`String::lines()` 返回 `Lines` 行迭代器（`next() -> Option<String>`，按 `\n`/`\r\n` 分行剥 `\r`），接入 `for` 与 V3 适配器框架；`chars_iter()`/`lines_iter()` 保留为等价别名（此前为码点级迭代器入口）** |
| region 选项 | ✅ `adaptive`/`with_size (N)`/`strategy (bump)` 已实现（L3，§3.4：region 指令接线 `rlyeh-region-alloc` C ABI 运行时 + `--profile` PGO 画像回灌 `adaptive` 初始容量）；`strategy (pool)` 等规划中 |
| `String::from(s)` | ✅ 支持字面量（及绑定字面量的变量）、运行期 `String` 变量（`≡ s.clone()` 深拷贝）、`&str` 视图（读 data/len 槽深拷贝）；G2 已消除长度表达限制 |

---

### 3.9a 集合宏（I3 ✅）

```rlyeh
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

```rlyeh
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

V1 瘦指针迭代器（2026-08，详见 development-plan.md §6.3c.3 T1a/V1）：

```rlyeh
// Vec::iter() / iter_mut() → 零分配零拷贝视图（裸指针 + 剩余长度）
let v = vec![1, 2, 3, 4];
let mut s = 0;
for x in v.iter() { s += x; }          // 10：值拷贝读取
let f = v.iter().filter(|x| x % 2 == 1).map(|x| x * 10); // Vec: [10, 30]：适配器链
let mut w = vec![1, 2, 3];
let mut it = w.iter_mut();
it.next(); it.write(99);               // 写回真实原槽：w = [99, 2, 3]
```

迭代器基于 `&self.data[0]` 真实取址（V1 GEP）构造瘦指针，`next()` 值拷贝读取推进，`IterMut::write(x)` 经 DerefSet 写回最近 next 读取的元素；编译器特判构造 `Iter::new`/`IterMut::new`。目标 API（std-lib §2.3/§3.1）仍为借用迭代器（引用元素 `Option<&T>`）+ 泛型元素。

---

### 3.11 堆分配 / 引用计数 / 可选 GC `Box<T>` / `Rc<T>` / `Arc<T>` / `Gc<T>`（K2–K4）

```rlyeh
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

// K4 Gc<T> 追踪 GC（MVP 保守标记-清除，rlyeh-gc-runtime）：
// gc_region 块内分配，块结束触发 GC 周期（未逃逸对象回收）
gc_region {
    let a = Gc::new(42);
    println(*a);            // 42
}

// 逃逸对象：块返回值存活到块外（rlyeh_gc_escape 登记为 root）
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

MVP 约束：`Box<T>`/`Rc<T>`/`Arc<T>`/`Weak<T>`/`Gc<T>` 为编译器内建（无 std 结构体定义，typecheck 特判）；`Box<T>` 布局 = 栈上 1 指针槽 + 堆上 `slot_count(T)` 个 8 字节槽；`Rc<T>` 布局 = 堆 `RcInner` 的 `T` 值区自堆首槽起（与 Box 同构）+ 尾部两计数槽（strong = 值区槽数、weak = +1），`Rc<T>` 栈上 1 槽指向 RcInner（详见 memory-model.md §4 MVP 注记）；`Gc<T>` 栈上 1 槽指向堆 1-槽包装（槽 0 存 `GcInner` 基址），对象与 `Box` 同构（详见 memory-model.md §5 MVP 注记）；聚合 T 装箱整槽区 memcpy（浅拷贝）；无自动 drop（计数只增不减，显式释放语义规划，与 `Vec`/`String` 一致）；`gc_region` 块 desugar 为 `rlyeh_gc_region_begin`/`rlyeh_gc_alloc`/`rlyeh_gc_escape`/`rlyeh_gc_collect`，块外对象永不回收、跨块逃逸对象引用图泄漏至程序结束，多线程 / 增量回收 / write barrier 规划中。

### 3.12 JSON 序列化 / 反序列化（L2 ✅）

`json::stringify(v)` 序列化 / `json::parse::<T>(s)` 反序列化——**编译器内建**，typecheck 期 desugar 为 String 构建/解析表达式，**零新增 IR 节点**。

```rlyeh
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
- 转义函数 `json_escape`/`json_unescape` 实现于 std `core.rl`；LLVM 字符串常量 `"` 用十六进制 `\22`（`\"` 被 clang 误解析为 `[1 x i8]` 长度不匹配）。
- MVP 限制：`map![...]` 绑定后 K/V 为 `Infer`，须 `let m: HashMap<i64, i64>` 注解（与 `vec![...]` 一致）；HashMap parse 键/值含逗号或冒号时 `split(",")`/`find(":")` 分段不可靠、嵌套 `HashMap` 值报 Unsupported（值限标量）；自定义 `Serialize`/`Deserialize` trait 与 `#[derive]` 宏规划中（std-lib §9）。

---

## 4. 编译器架构

### 4.1 编译流水线

```
源代码 (.rl)
    │
    ▼
┌─────────────────────────────────────┐
│  Lexer (rlyeh-lexer)                │  → Token 流
├─────────────────────────────────────┤
│  Parser (rlyeh-parser)              │  → AST
├─────────────────────────────────────┤
│  Name Resolution & Macro Expand     │  → HIR
├─────────────────────────────────────┤
│  Type Checker (rlyeh-typecheck)      │  → 类型标注的 HIR
├─────────────────────────────────────┤
│  Borrow Checker (rlyeh-borrowck)     │  → 借用验证
├─────────────────────────────────────┤
│  Region Checker (rlyeh-regionck)     │  → 区域验证 + 大小推断
├─────────────────────────────────────┤
│  MIR Lowering (rlyeh-mir)           │  → 控制流图
├─────────────────────────────────────┤
│  Optimization Passes                │  → 常量折叠、内联、死代码消除
├─────────────────────────────────────┤
│  LIR Lowering (rlyeh-lir)           │  → 目标无关低级 IR
├─────────────────────────────────────┤
│  Code Generation (rlyeh-codegen)     │  → LLVM IR / Cranelift IR
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
| 模块级缓存 | 每个 .rl 文件 | 接口哈希未变 |
| 函数级并行 | 无依赖函数 | 编译图分析 |
| 类型检查缓存 | 类型推导结果 | 输入类型未变 |
| PGO 数据复用 | 区域大小预测 | .rl_profile 存在 |

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
| `rlyeh new <name>` | 创建新项目（`--lib` 库项目；生成 Rlyeh.toml + src/main.rl 或 lib.rl） | ✅ 可用（委托 dagon 脚手架） |
| `rlyeh build` | 编译项目 | ✅ 可用（MVP；`--target <triple>` 交叉编译，macOS 双架构已验证） |
| `rlyeh run` | 编译并运行 | ✅ 可用（MVP） |
| `rlyeh test` | 运行测试 | ✅ 可用（D1：`tests/` 目录 compile-pass/compile-fail/run-pass） |
| `rlyeh fmt` | 代码格式化 | ✅ 可用（D2：AST 重建，`--check`/`-w`/`--indent`） |
| `rlyeh check` | 静态分析 | ✅ 可用（D2：未使用变量/恒常条件/冗余比较/不可达代码） |
| `rlyeh bench` | 基准测试 | ✅ 可用（D3：编译 + 多次计时统计，`--runs`/`--warmup`） |
| `rlyeh doc` | 生成文档 | ✅ 可用（D3：`///` 注释提取 → Markdown，`--out`/`--title`） |
| `rlyeh publish` | 发布包 | ✅ 可用（E3：委托 dagon 打包发布，`--registry`/`--verbose`；重复版本拦截） |
| `rlyeh lsp` | 语言服务器 | ✅ 可用（F1：LSP over stdio，full 文档同步 + 诊断推送，复用 rlyeh-check 静态分析） |
| `rlyeh profile` | PGO 回灌报告 | ✅ 可用（F2：`.rl_profile` → 区域大小预测报告；`rlyeh build --profile` 编译期注入） |

---

## 5.5 编译器加固与标准库扩展（新版开发计划执行记录，2026-08）

> 本节记录新版开发计划（阶段 A–F，详见 [`docs/development-plan.md`](docs/development-plan.md)）的执行进度：
> **阶段 A 全部完成（A1–A4 ✅），阶段 B 全部完成（B1–B5 ✅），阶段 C 全部完成（C1–C3 ✅），阶段 D 全部完成（D1–D3 ✅：rlyeh test / rlyeh fmt / rlyeh check / rlyeh doc / rlyeh bench），阶段 E 全部完成（E1 交叉编译 `--target` macOS 双架构 + `__rlyeh_target_os` 平台内建消除 `sockaddr_in4` 布局假设；E2 WASM 目标：`--target wasm32-wasi` 编译 + wasmtime 运行验证；E3 发布流程：dagon publish 重复版本保护 + `rlyeh publish` CLI + release.yml 四平台 + CHANGELOG.md），阶段 F 全部完成（F1 LSP 服务器 MVP：`rlyeh lsp` + 新 crate `rlyeh-lsp`，文档同步 + 诊断推送；F2 PGO 数据回灌：`rlyeh profile` 命令 + `rlyeh build --profile` 编译期注入，`.rl_profile` → 区域大小预测报告）**，E1 Windows/ARM 工具链（待对应环境）待做。
> **后续计划（编译器能力 G–L 已完成；标准库深度完善 M–T 已完成；U1 作用域栈 ✅；U2 关联类型 ✅；U3 泛型约束 ✅；U4 Self 返回 ✅；U5 AddrOf ✅；U6 Cast IR ✅——U 编译器地基阶段全部完成）**：见 [`docs/development-plan.md`](docs/development-plan.md)（§6.3 阶段 G–L 编译器能力补齐 + §6.3b 阶段 M–T 标准库深度完善——错误处理基底 M ✅ → 文件系统 N ✅ → 网络 O → 并发通道 P → 序列化/格式化 trait Q → 高性能 IO R → 异步运行时 S（前置：线程支持 S0）→ 集合收尾 T）；**目标 API 对齐计划（阶段 U–Z）见 §6.3c**——U 编译器地基（**作用域栈 ✅ U1 已完成**/**关联类型 ✅ U2 已完成**/**泛型约束 ✅ U3 已完成**/**`Self` 返回 ✅ U4 已完成**/**AddrOf ✅ U5 已完成**/**Cast IR ✅ U6 已完成**）→ **V 集合与迭代器完整化（V1 借用迭代器瘦指针 MVP ✅ / V4 `get_mut` 引用语义 ✅ / V5 新集合 HashSet+BTreeMap+VecDeque ✅ / V3 trait 默认方法机制 + Iterator 默认方法 ✅ 2026-08-26）** → W 异步运行时完整化 → X 序列化/格式化/时间完整化（X1 时间 API ✅） → Y IO/网络/并发/智能指针收尾。

> 阶段 A–F / G–L / M–T / U–Z 的任务具体执行情况已归档至任务树（阶段索引文档 §子任务叶子文档，每个任务一个叶子）：
> - 阶段 A–F：[`docs/tasks/stage-a-f.md`](docs/tasks/stage-a-f.md)
> - 阶段 G–L：[`docs/tasks/stage-g-l.md`](docs/tasks/stage-g-l.md)
> - 阶段 M–T：[`docs/tasks/stage-m-t.md`](docs/tasks/stage-m-t.md)
> - 阶段 U–Z：[`docs/tasks/stage-u-z.md`](docs/tasks/stage-u-z.md)
>
> 本文件 §5.5 原为逐条执行记录，已按 §7.4 任务管理体系迁移至任务树；详细实现流水见任务树 / git 历史。

## 6. 里程碑列表

> 本项目按 3 个里程碑规划。各里程碑的任务列表见任务树 [`milestone-1/2/3.md`](docs/tasks/README.md)（每个任务一个单任务文档，见 `docs/tasks/milestone-tasks/`）。

| 里程碑 | 范围 | 状态 | 任务列表文档 |
|--------|------|------|-------------|
| **Milestone 1** — 编译器 MVP（Month 0-4） | M1.1 Lexer → M1.9 hello-world | ✅ 全部完成 | [`milestone-1.md`](docs/tasks/milestone-1.md) |
| **Milestone 2** — 生产可用（Month 4-8） | M2.1 Actor 运行时 → M2.8 智能区域 | 🔧 部分完成（M2.4 LSP 待；M2.5-2.8 完成） | [`milestone-2.md`](docs/tasks/milestone-2.md) |
| **Milestone 3** — 生态繁荣（Month 8-12） | M3.1 数据库 → M3.6 官方教程 | 📋 规划（未开始） | [`milestone-3.md`](docs/tasks/milestone-3.md) |

---


## 7. 编码规范

### 7.0 文件大小约束（铁律，必须遵守）

> **这是最高优先级的硬性约束，任何情况下不得违反。**

- **单个代码文件的最大行数为 800–1000 行**（`crates/` 下的 Rust 源码与 `crates/rlyeh-std/rlyeh/` 下的 `.rl` 标准库均适用；纯文档 `.md` 不适用）。
- 当文件接近该上限时，**必须立即拆分**：按职责/模块/函数簇抽离为多个独立文件或子模块，并保持公共 API 与调用点不变。
- 新增代码**不得**让已有文件超过 1000 行；新增逻辑应放入新文件或拆分后的模块。
- **违反此约束视为缺陷**，代码评审必须拒绝超过上限的改动，并优先修复既有超限文件（逐步拆分回归至 1000 行内）。
- 拆分时保持语义等价，拆完后必须通过全量回归（`rlyeh test` + `cargo test`）。

**超限文件拆分进度（2026-08-27）**：9 个超限 Rust 文件已全部拆分回归 ≤1000 行；`core.rl` 为编译器预置的单一语言源码（类型同一命名空间互引用），直接拆分需改 stdlib 注入机制 + 测试内联副本，暂不拆分（专项处理）。

| 文件 | 拆分前 | 拆分后（≤1000 行） | 状态 |
|------|:---:|---------|------|
| `crates/rlyeh-typecheck/src/check_expr.rs` | 11779 | `check_expr/mod.rs` + iter/binary/call/resolve/construct/field/heap/index_enum/method/generic/macro_serialize/util（12 文件） | ✅ 已完成 |
| `crates/rlyeh-codegen/src/llvm.rs` | 3259 | `llvm.rs`(壳) + `llvm/llvm_call/ctor/emit/func/region/util`（6 文件） | ✅ 已完成 |
| `crates/rlyeh-desugar/src/analyze.rs` | 1933 | `analyze/mod.rs` + expand/scan/segment | ✅ 已完成 |
| `crates/rlyeh-desugar/src/generate.rs` | 1451 | `generate/mod.rs` + gen_poll/rewrite/devar | ✅ 已完成 |
| `crates/rlyeh-driver/src/lib.rs` | 1462 | `lib.rs` + platform_ir.rs + util.rs | ✅ 已完成 |
| `crates/rlyeh-parser/src/expr.rs` | 1384 | `expr/mod.rs` + primary/control/macro_ | ✅ 已完成 |
| `crates/rlyeh-typecheck/src/check_item.rs` | 1226 | `check_item/mod.rs` + collect/actor/fn_sig | ✅ 已完成 |
| `crates/rlyeh-lir/src/lower.rs` | 1168 | `lower/mod.rs` + lower_stmts.rs | ✅ 已完成 |
| `crates/rlyeh-parser/src/tests.rs` | 1108 | `tests/mod.rs` + region/control/decl | ✅ 已完成 |
| `crates/rlyeh-std/rlyeh/core.rl` | 2186 | —（预置单一语言源码，暂不拆分） | ⏸ 专项处理 |
| `tools/rlyeh-fmt/src/lib.rs` | 1316 | `lib.rs` + `fmt_expr` + `fmt_pattern`（3 文件：762/362/211） | ✅ 已完成（2026-09-02） |
| `crates/rlyeh-codegen/src/llvm.rs`（二次拆分） | 1068 | region 分支 → 既有 `llvm/llvm_region.rs`（361→492）；字段/索引/指针/解引用分支 → 新增 `llvm/llvm_field.rs`（543）；`llvm.rs` 461 | ✅ 已完成（2026-09-02） |

**复现超限（2026-09-02 扫描 + 同日拆分完毕）**：以下 5 个文件曾再次越过 1000 行，已于同日按簇下沉至子模块，均已回归 ≤1000 行：

| 文件 | 拆分前 | 拆分后 | 状态 |
|------|:---:|--------|------|
| `crates/rlyeh-typecheck/src/check_expr/toml.rs` | 1444 | `toml.rs` 939 + `toml/try_parse` 241 + `toml/build` 283 | ✅ 已完成（2026-09-02） |
| `crates/rlyeh-typecheck/src/check_expr/method.rs` | 1433 | `method.rs` 924 + `method/thread` 331 + `method/dyn_call` 196 | ✅ 已完成（2026-09-02） |
| `crates/rlyeh-driver/src/lib.rs`（二次超限） | 1208 | `lib.rs` 546 + `link` 679 | ✅ 已完成（2026-09-02） |
| `crates/rlyeh-typecheck/src/check_expr/construct.rs` | 1124 | `construct.rs` 606 + `construct/collection` 293 + `construct/string` 243 | ✅ 已完成（2026-09-02） |
| `crates/rlyeh-typecheck/src/check_expr/mod.rs` | 1082 | `mod.rs` 595 + `ctrl` 515（`infer_expr` 尾部分支下沉） | ✅ 已完成（2026-09-02） |

> 至此 `crates/`、`tools/`、`dagon/` 下已无超过 1000 行的 Rust 源文件
> （2026-09-02 全量扫描确认）。`toml.rs` / `method.rs` 仍接近上限（939 / 924），
> 二者主体各是一个大函数（`toml_parse_ast` / `check_method_call`），
> 若后续继续增长应优先按阶段抽取而非整函数下沉。

> **拆分规范**：保持语义等价；`mod`/`use` 改为子模块（`mod xxx;` + `use xxx::*`，子模块私有函数提升为 `pub(super)`/`pub(crate)`，对外 API 从 mod.rs 显式 re-export）；每个文件拆分后须通过全量回归（`rlyeh test` 194 用例 + `cargo test`）。新增代码一律不得再扩大超限文件。

### 7.1 Rust 代码（编译器实现）

- 遵循 `rustfmt` 默认配置
- 每个 crate 必须有 `lib.rs` 或 `main.rs`
- 公共 API 必须有文档注释（`///`）
- 测试覆盖率目标：核心模块 ≥ 80%

### 7.2 Rlyeh 代码（标准库 + 示例）

- 使用 `rlyeh fmt` 格式化（D2 已实现：`rlyeh fmt <file.rl> [-w]`）
- 文件扩展名：`.rl`
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

### 7.4 任务管理体系（策划约束，强制）

> **本约束对 AI 与所有参与者一视同仁**：此后策划/规划任何新任务（新阶段、新里程碑、新功能、新修复）时，**必须**按本体系在 `docs/tasks/` 下组织任务文档并进行风险拆分，不得只写进本文件或散落于计划文档。

#### 7.4.1 树形任务文档结构

任务文档统一存放于 `docs/tasks/`，按**树形层级**组织，每个文档只承担固定职责：

```
docs/tasks/
├── README.md                  # 树根：里程碑列表入口 + 全部阶段索引 + 总进度
├── milestones.md              # 里程碑总览：整合各阶段（A–Z）里程碑 + 进度 + 已知问题
├── milestone-1/2/3.md         # 里程碑任务列表：该里程碑细化后的任务列表（编号|内容|状态|单任务文档）
├── milestone-tasks/           # 里程碑单任务文档：每个里程碑任务一个文档（m1-1.md…m3-6.md，含执行情况/技术细节）
├── stage-<组>.md              # 阶段索引：该阶段任务列表 + 进度 + 子任务叶子清单
├── <任务>-<索引>.md            # 任务索引：该任务子任务列表 + 依赖 + 风险 + 进度
└── leaf/                      # 叶子层：每个最小粒度子任务一个文档
```

职责约定：
- **上层文档只记录下一级任务文档的任务列表与实现进度**，不承载具体实施细节。
- **叶子文档**承载最小粒度单个任务的具体实施（目标 / 背景 / 改动范围 / 验证 / 状态 / 变更记录）。
- 阶段/任务的具体实施可链接到权威源文档（`docs/development-plan.md`）或叶子文档，避免重复维护。

#### 7.4.2 任务策划流程（新任务必须执行）

策划新任务时按以下顺序进行，每一步都产出对应任务文档：

1. **定位层级**：确定该任务归属的阶段 → 在 `stage-<组>.md` 登记阶段进度；新阶段则新建阶段索引。
2. **建立任务索引**：在 `docs/tasks/` 新建 `<任务>-<索引>.md`，列出子任务列表（子任务名 / 叶子文档 / 依赖 / 风险 / 状态）。
3. **拆分叶子（强制：一个任务对应一个文档）**：为**每一个最小粒度任务**在 `leaf/` 新建独立文档（一个文档只对应一个任务），含具体实施四要素（目标 / 技术细节 / 验证 / 状态）+ 变更记录。**不允许任何任务只停留在 stage 索引或执行记录段落而无叶子文档**。
4. **更新同步**：更新树根 `README.md`、`milestones.md` 及上级索引的总进度。

#### 7.4.3 风险分级与强制拆分（核心约束）

每个任务/子任务必须标注**风险等级**（低 / 中 / 高），并遵循以下规则：

| 风险 | 判定 | 处理 |
|------|------|------|
| 低 | 纯收尾、测试补充、独立小改动，失败影响面小 | 保持单任务即可 |
| 中 | 涉及局部机制，独立可测、可回退 | 保持单任务即可 |
| **高** | 类型系统结构性改动、集中式分派重构、跨多模块大面积改动、未知机制 | **必须**进一步细化为多个中/低风险子任务 |

**强制拆分规则**：
- 规划时若某任务风险为**高**，必须拆分为多个**中/低**风险子任务，才允许排期实施。
- 拆分依据：将「结构性改动」按序拆为「载体 → 解析 → 落地 → 收尾」小步；将「集中式重构」拆为「每对独立改动 + 收敛 + 清理」。
- 拆分后每个子任务须**独立可测、失败可回退**，并在任务索引中更新依赖与风险。
- 已完成的高风险任务不需再拆（保留「高 ✅」标注）。

#### 7.4.4 实施进度跟踪

- 叶子文档的「状态」字段（📋 规划 / 🔧 进行中 / ✅ 已完成）随实施实时更新。
- 每次完成子任务后，向上逐级同步：叶子 → 任务索引 → 阶段索引 → `milestones.md` → 树根 `README.md`。
- 本文件 §6 仅保留里程碑总览摘要；**里程碑执行情况**已归档至 [`docs/tasks/milestone-1/2/3.md`](docs/tasks/README.md)；**任务列表、进度与具体执行记录一律以 `docs/tasks/` 树为准**（阶段索引 §子任务叶子文档，每个任务一个叶子），§5.5 不再承载逐条执行记录。

---

## 8. 测试策略

### 8.1 单元测试

每个 crate 内 `tests/` 目录或 `#[test]` 标注。

```rust
// crates/rlyeh-lexer/src/lib.rs
#[test]
fn test_comparison_chain_tokens() {
    let source = "if 0 < x < 10 {}";
    let tokens = lex(source);
    assert_eq!(tokens.len(), 8); // if, 0, <, x, <, 10, {, }
}
```

### 8.2 集成测试

`tests/` 目录下的 `.rl` 文件，通过 `rlyeh test` 运行。

```
tests/
├── compile-pass/    ← 应该编译通过的用例（编译到 LLVM IR，不运行）
│   ├── hello.rl / arith.rl / struct-trait.rl
│   ├── modules.rl / actor.rl
├── compile-fail/    ← 应该编译失败的用例（`// expect: <片段>` 断言错误消息）
│   ├── type-mismatch.rl / undefined-var.rl / unknown-field.rl
│   └── undefined-fn.rl / no-method.rl
└── run-pass/         ← 编译并运行，同名 `.out` 文件精确对比输出
    ├── hello.rl (+ hello.out)
    ├── arith.rl (+ arith.out)
    └── actor-ping-pong.rl (+ actor-ping-pong.out)
```

### 8.3 基准测试

```rust
// crates/rlyeh-lexer/benches/lexer_bench.rs
use criterion::{black_box, Criterion};

fn bench_large_file(c: &mut Criterion) {
    let source = std::fs::read_to_string("large_input.rl").unwrap();
    c.bench_function("lex_large_file", |b| {
        b.iter(|| lex(black_box(&source)))
    });
}
```

### 8.4 模糊测试

```rust
// crates/rlyeh-parser/fuzz/fuzz_target.rs
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
5. 更新 `dagon` 注册表中的版本信息

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

> **最后更新**：2026-08-31  
> **维护者**：Rlyeh Language Team  
> **License**：MIT / Apache-2.0
