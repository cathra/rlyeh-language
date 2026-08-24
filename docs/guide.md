# Zeta 编程语言指南

> 版本：v2.0（MVP）
> 本文为面向读者的语言教程。所有示例均为 `examples/`、`tests/run-pass/` 中可编译运行的已验证代码（或其简化）。
> 权威规范见：[grammar.md](./grammar.md)（EBNF）、[semantics.md](./semantics.md)（语义）、[memory-model.md](./memory-model.md)、[actor-model.md](./actor-model.md)、[std-lib.md](./std-lib.md)。

---

## 目录

1. [认识 Zeta](#1-认识-zeta)
2. [快速上手](#2-快速上手)
3. [基础语法](#3-基础语法)
4. [数学式条件判断](#4-数学式条件判断)
5. [聚合类型与泛型](#5-聚合类型与泛型)
6. [数组与索引](#6-数组与索引)
7. [模块系统](#7-模块系统)
8. [内存管理：分层所有权模型](#8-内存管理分层所有权模型)
9. [Actor 并发模型](#9-actor-并发模型)
10. [标准库](#10-标准库)
11. [编译目标与工具链](#11-编译目标与工具链)
12. [外部函数接口（FFI）](#12-外部函数接口ffi)
13. [参考与已知限制](#13-参考与已知限制)

---

## 1. 认识 Zeta

Zeta 是一门面向未来十年基础设施的**系统级编程语言**：

| 设计目标 | 对标 | 落地形态 |
|----------|------|----------|
| 内存安全、零 GC | Rust | 分层内存模型（L0 所有权 → L1 区域 → L2 Rc → L3 GC） |
| 编译速度极快 | Go | 模块级缓存（增量编译，已实现）、函数级并行（规划中） |
| 并发模型一等公民 | Erlang / Akka | `actor` 语言级构造 + 运行时监督 |
| 数学式语法直觉 | Python / MATLAB | 比较链 `0 < x < 10`、集合判断 `x in (1, 3, 5)` |

**一句话定位**：Zeta = Rust 的安全性 + Go 的编译速度 + Erlang 的并发模型 + 数学的自然表达。

当前编译器为 Rust 实现的 bootstrap 阶段（自举目标），代码生成走 LLVM IR。

---

## 2. 快速上手

### 2.1 第一个程序

```zeta
// hello-world.zeta
fn main() {
    println("Hello, Zeta!");
}
```

### 2.2 编译与运行

```bash
zeta build hello-world.zeta     # 生成可执行文件 hello-world
zeta run hello-world.zeta       # 编译并运行
zeta test                       # 运行 tests/ 目录 compile-pass/compile-fail/run-pass 用例
```

### 2.3 工具链速览

| 命令 | 功能 |
|------|------|
| `zeta new <name> [--lib]` | 创建新项目脚手架（Zeta.toml + src/main.zeta 或 lib.zeta） |
| `zeta build <file> [-o <out>] [--target <triple>]` | 编译为可执行文件（`--target` 交叉编译 / WASM / `--profile` 注入 PGO） |
| `zeta run <file>` | 编译并运行 |
| `zeta test` | 运行测试目录用例 |
| `zeta fmt <file>` | 代码格式化（`--check` / `-w` / `--indent`） |
| `zeta check <file>` | 静态分析（未使用变量 / 恒常条件 / 冗余比较 / 不可达代码） |
| `zeta bench <file>` | 基准测试（`--runs` / `--warmup`） |
| `zeta doc <file>` | 从 `///` 注释生成 Markdown 文档 |
| `zeta publish [--registry] [--verbose]` | 打包发布到 zep 注册表（重复版本拦截） |
| `zeta lsp` | 语言服务器（LSP over stdio，诊断推送） |
| `zeta profile <file.zeta_profile>` | PGO 画像 → 区域大小预测报告（`--out`） |

---

## 3. 基础语法

### 3.1 变量与类型

`let` 声明不可变绑定；标量类型：`i8/i16/i32/i64/isize`、`u8/u16/u32/u64/usize`、`f32/f64`、`bool`、`char`、`()`。

```zeta
let x = 6;                       // 类型推断：i64
let y: f64 = 3.14;               // 显式类型注解
let mut v = Vec::new();          // 可变绑定（对象修改用）
```

### 3.2 函数

```zeta
fn add(a: i64, b: i64) -> i64 {
    a + b                        // 末表达式为返回值（隐式 return）
}

fn main() {
    let sum = add(6, 7);         // 13
}
```

#### 函数一等值（函数指针，H1）

函数可以像值一样绑定、传递与调用：

```zeta
fn apply(f: fn(i64, i64) -> i64, x: i64, y: i64) -> i64 {
    f(x, y)                      // 通过函数值间接调用
}

fn main() {
    let f = add;                 // 函数值绑定（推断为 fn(i64, i64) -> i64）
    let r1 = f(3, 4);            // 7
    let r2 = apply(add, 10, 20); // 函数值作实参：30

    let g: fn(i64, i64) -> i64 = add;  // 显式类型注解
    let r3 = g(5, 6);            // 11：注解变量间接调用

    let f2 = f;                  // 重新绑定
    let r4 = f2(7, 8);           // 15
}
```

##### 无捕获闭包（H2）

闭包 `|x, y| 表达式` desugar 为匿名函数 + 函数指针（零运行时开销），参数类型由 fn 上下文推断：

```zeta
fn apply(f: fn(i64, i64) -> i64, x: i64, y: i64) -> i64 {
    f(x, y)                      // 通过函数值间接调用
}

fn main() {
    let r1 = apply(|a, b| a + b, 10, 20);  // 30：闭包作 fn 形参实参
    let inc: fn(i64) -> i64 = |x| x + 1;   // fn 注解绑定闭包
    let r2 = inc(41);            // 42：经函数值间接调用
    let r3 = apply(|a, b| a * 2 + b * 3, 1, 2);  // 8
}
```

MVP 约束（见 §13）：闭包体仅可引用参数与字面量；参数模式仅支持简单标识符与 `_`；返回闭包的函数暂不支持。

##### 捕获闭包（H3，IIFE MVP）

捕获闭包 `(|x, y| 表达式)(实参)` 以**立即调用**形式使用：闭包体引用的外层变量**按值捕获**，desugar 为匿名函数（捕获变量作前置参数、参数名保留原名）+ 普通函数调用，零新增 IR 节点：

```zeta
fn main() {
    // 单捕获：闭包体引用外层变量 factor
    let factor = 3;
    let r1 = (|x| x * factor)(14);              // 42

    // 多捕获 + 字符串拼接（String 语义）
    let name = String::from("zeta");
    let r2 = (|s| s + name)(String::from("hi "));  // hi zeta

    // 参数遮蔽捕获（同名参数优先）
    let v = 100;
    let r3 = (|v| v + 1)(7);                    // 8

    // 捕获数组变量做索引
    let base = [1, 2, 3];
    let r4 = (|i| base[i] * 2)(1);              // 4
}
```

MVP 约束（见 §13）：支持立即调用形式、闭包值对象（见 H5）与返回闭包的函数（H5 补全）；按值捕获（`move` 关键字 MVP 忽略，所有权宽松）；IIFE 闭包参数无类型注解（类型从实参推断）；不支持嵌套捕获闭包（外层闭包体内直接写 IIFE 捕获内层可运行）。

##### 闭包值对象（H5）

`let f = |x: i64| 表达式;` 将闭包绑定为**值对象**，之后可反复调用 `f(实参)`：闭包体引用的外层变量**按值捕获**，desugar 为匿名函数 + 捕获聚合对象（每捕获一槽）+ 调用点展开（从聚合对象读取捕获字段传给匿名函数）。全部为既有 IR 原语（`Alloc` / `FieldSet` / `FieldGet` / `Call`），零新增 IR 节点。**参数类型确定规则**（H5 补全）：有类型注解的参数用注解类型（调用点实参须兼容），无注解的参数由**首次调用点实参推断**（延迟固化；`let f = |x| x + 1; f(41);` 直接可用，半注解 `|x: i64, y|` 亦可用），从未被调用则闭包体不检查（惰性）：

```zeta
fn main() {
    // 基本绑定 + 调用
    let inc = |x: i64| x + 1;
    let r1 = inc(41);                        // 42

    // 按值捕获（捕获时类型快照；后续外层修改不影响闭包内拷贝）
    let base = 40;
    let add_base = |x: i64| x + base;
    let r2 = add_base(2);                    // 42

    // 多参数 + 多捕获 + 闭包值别名（聚合指针拷贝后仍可调用）
    let a = 30;
    let b = 12;
    let f = |x: i64, y: i64| x + y + a + b;
    let g = f;
    let r3 = g(0, 0);                        // 42

    // 捕获 String 对象 + 闭包体内方法调用
    let s = String::from("hello");
    let f9 = |x: i64| x + s.len();
    let r4 = f9(37);                         // 42

    // String 实参升级（字面量 → String，与 IIFE 一致）
    let f10 = |s: String| s.len() + 37;
    let r5 = f10("hello");                   // 42

    // 闭包体内局部变量（块表达式闭包体）
    let f12 = |x: i64| { let y = x * 2; y };
    let r6 = f12(21);                        // 42
}
```

H5 补全——非注解/半注解绑定（参数由首次调用点实参推断）、返回闭包的函数、闭包值作 fn 实参：

```zeta
fn apply(f: fn(i64) -> i64, x: i64) -> i64 { f(x) }

// 返回闭包的函数：尾表达式为无捕获闭包 → 按 H2 签名检查生成函数指针
fn make() -> fn(i64) -> i64 { |x| x + 1 }

// 返回闭包的函数：无捕获闭包值变量作返回值 → 降级为 fn 指针
fn make2() -> fn(i64) -> i64 {
    let f = |x: i64| x * 2;
    f
}

// 返回闭包的函数：未固化闭包作返回值 → 按返回签名固化参数类型
fn make3() -> fn(i64) -> i64 {
    let f = |x| x * 3;
    f
}

fn main() {
    // 非注解绑定：参数类型由首次调用点实参推断（延迟固化）
    let f = |x| x + 1;
    println(f(41));                          // 42
    println(f(1));                           // 2

    // 半注解：x 注解 i64，y 由实参推断
    let g = |x: i64, y| x + y;
    println(g(20, 22));                      // 42

    // 非注解 + 按值捕获（多次调用）
    let base = 40;
    let h = |x| x + base;
    println(h(2));                           // 42

    // 非注解闭包值作 fn 实参（按 fn 形参签名固化）
    let c = |x| x * 3;
    println(apply(c, 14));                   // 42

    // 返回闭包的函数 + fn 实参传递
    let m = make();
    println(apply(m, 41));                   // 42
    let m3 = make3();
    println(apply(m3, 14));                  // 42
}
```

MVP 约束（见 §13）：参数有注解用注解、无注解由首次调用点实参推断（半注解亦可用，从未调用则惰性不检查；注解与推断/签名冲突报类型不匹配）；仅按值捕获；**无捕获闭包值可作 fn 实参/返回值**（降级为函数指针或按 fn 签名固化参数类型）；捕获闭包值不跨函数边界传递（作 fn 实参/返回值报 Unsupported/类型不匹配）；不支持嵌套捕获闭包。

##### trait 对象（H4，`dyn Trait`）

`dyn Trait` 是 trait 对象类型：**2 槽胖指针**（数据指针 + vtable 指针），`&T`（具体类型引用）可强制转换为 `dyn Trait`——desugar 为运行时构造 vtable（drop/size/align 槽 MVP 置 0 + 方法表）+ 胖指针。方法调用经 vtable 间接分派，同一签名可分派到不同 impl：

```zeta
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

fn main() {
    let c = Circle { radius: 2.0 };
    let r = Rect { w: 3.0, h: 4.0 };

    let d1: dyn Shape = &c;          // &T → dyn Trait 强制转换
    let d2: dyn Shape = &r;
    println(d1.area());              // 12.56（vtable 分派到 Circle::area）
    println(d2.area());              // 12.0（分派到 Rect::area）
    println(d1.sides());             // 0
    println(d2.sides());             // 4

    let d3 = d1;                     // 胖指针拷贝共享同一 vtable
    println(d3.area());              // 12.56
}
```

MVP 约束（见 §13）：trait 与 impl 均须非泛型；方法签名含 `Self`（关联返回类型 / 参数）不支持经 dyn 调用；vtable 的 drop/size/align 槽置 0（显式释放语义与 `Box`/`Rc` 一致）。

**去虚拟化（2026-08-24 ✅）**：`let d: dyn Trait = &obj;` 绑定变量时记录来源具体类型，后续 `d.method()` 静态分派到具体类型实现（经 `instantiate_impl_method` 取 mono 符号，含模块前缀 / 泛型实例化），LLVM 可内联 / 常量折叠——`dyn_dispatch` 基准 15.5ms → 3.6ms，与 Rust（rustc -O 去虚拟化）持平（1.08x）。保守回退：dyn 变量被重新赋值（`d = ...`）时映射失效，自动回退 vtable 间接调用，多态语义不变；仅局部 `let` 绑定变量适用（H4 本就限定 dyn 仅局部变量）。

##### `?` 错误传播运算符（K1）

`expr?` 在 `Option<T>` / `Result<T, E>` 上下文解包成功值，失败时从当前函数早返回失败值：

```zeta
fn try_div(x: i64, y: i64) -> Option<i64> {
    if y == 0 {
        return None;
    }
    Some(x / y)
}

fn chain(a: i64, b: i64, c: i64) -> Option<i64> {
    let q = try_div(a, b)?;        // 解包；失败则 `return None`
    let r = try_div(q, c)?;
    Some(r + 1)
}

fn sum_all(a: i64, b: i64, c: i64) -> Option<i64> {
    Some(try_div(a, b)? + try_div(c, 2)?)   // 表达式中间嵌套 ? 也可
}
```

- `Option<T>`：desugar 为 `match expr { Some(v) => v, None => return Option::None }`
- `Result<T, E>`：desugar 为 `match expr { Ok(v) => v, Err(e) => return Result::Err(e) }`
- 裸无参变体值可用：`return None;` 与 `Option::None` 等价
- MVP 约束：`?` 用于非 Option/Result 类型报错；返回类型兼容性检查与现有 `return` 语义一致（宽松）

##### 迭代器与适配器（J1–J3）

```zeta
// J1 数组迭代：for x in arr（索引遍历，长度编译期已知）
let arr: [i64; 4] = [1, 2, 3, 4];
let mut sum = 0;
for x in arr {
    sum += x;
}

// J2 自定义迭代器接入 for：类型存在 `next() -> Option<T>` 方法即可
struct Counter { limit: i64, pos: i64 }
impl Counter {
    fn new(limit: i64) -> Counter { Counter { limit: limit, pos: 0 } }
    fn next(&mut self) -> Option<i64> {
        if self.pos >= self.limit { return None; }
        let v = self.pos;
        self.pos += 1;
        Some(v)
    }
}
for v in Counter::new(5) { /* 0, 1, 2, 3, 4 */ }

// J3 适配器（数组 / Vec / 迭代器接收者，经 H2 无捕获闭包，返回 Vec<T>）
let d = [1, 2, 3].map(|x| x * 2);            // Vec: [2, 4, 6]
let e = [1, 2, 3, 4, 5].filter(|x| x % 2 == 1); // Vec: [1, 3, 5]
let t = [1, 2, 3, 4].fold(0, |acc, x| acc + x); // 10
let c = [7, 8, 9].collect();                  // Vec: [7, 8, 9]
let tk = [1, 2, 3, 4, 5].take(3);             // Vec: [1, 2, 3]
let sk = [1, 2, 3, 4, 5].skip(2);             // Vec: [3, 4, 5]
let chained = Counter::new(6).filter(|x| x > 1).map(|x| x * x); // Vec: [4, 9, 16, 25]
```

- 适配器语义（内建 desugar）：`map` 变换元素、`filter` 保留满足谓词的元素、`fold(init, |acc, x| ..)` 归约返回 `acc`、`collect` 原样收集、`take(n)` 取前 n 个、`skip(n)` 跳过前 n 个
- MVP 约束：适配器参数必须是无捕获闭包（捕获外部变量报错，H3 规划）；适配器返回 `Vec<T>`（急切求值），结果 Vec 可作下一适配器源（链式）；`Iterator` trait 定义（std-lib §2.3）为规划 API（适配器为编译器内建）

### 3.3 控制流

```zeta
if sum > 10 {
    println("sum is big");
};

let mut i = 0;
while i < 10 {
    i = i + 1;
}

loop {
    if done { break; }
    continue;
}

for i in 0..<10 {        // 数值区间循环：半开 [0, 10)，i 取 0..=9
    println(i);          // 0 1 2 ... 9
}

for j in 1...3 {         // 双闭区间 [1, 3]
    println(j);          // 1 2 3
}
```

`for` 支持数值区间（步长固定为 1）与 Vec / HashMap 容器迭代：`for x in vec`（见 §10.3）、`for (k, v) in map`（见 §10.4）；**数组迭代暂不支持**。`break` / `continue` 均可用。`match` 用于枚举解构（见 §5.2）。

### 3.4 注释

```zeta
// 行注释
/// 文档注释（`zeta doc` 提取生成 Markdown）
```

---

## 4. 数学式条件判断

### 4.1 区间内（正向比较链）

```zeta
if 0 < x < 10 {}      // 0 < x && x < 10
if 0 <= x <= 10 {}    // 0 <= x && x <= 10
if 0 < x <= 10 {}     // 0 < x && x <= 10
```

### 4.2 区间外（反向比较链）

```zeta
if 0 > x > 10 {}      // x < 0 || x > 10（反向外链 = 并集）
if 0 >= x >= 10 {}    // x <= 0 || x >= 10
```

### 4.3 集合判断（括号内为离散集合）

```zeta
if x in (1, 3, 5) {}                          // x == 1 || x == 3 || x == 5
if ch in ('a'..<'z', 'A'..<'Z') {}            // 范围元素展开为离散成员
if x in (0...10) {}                           // 等价 x == 0 || ... || x == 10
if x not in (0..<10) {}                       // 不等价于全部（!x in）
```

### 4.4 裸范围 = 区间判断

```zeta
if x in 0..<10 {}    // 0 <= x < 10     [0, 10)
if x in 0...10 {}    // 0 <= x <= 10    [0, 10]
if x in 0<..10 {}    // 0 < x <= 10     (0, 10]
```

### 4.5 时间字面量

```zeta
if hour in (9am...6pm) {}             // 工作时间段
if hour not in (6am..<10pm) {}        // 跨午夜自动处理
```

---

## 5. 聚合类型与泛型

### 5.1 结构体与字面量构造

```zeta
struct Point {
    x: i64,
    y: i64,
}

let p = Point { x: 1, y: 2 };
```

### 5.2 枚举与匹配

```zeta
enum Shape {
    Circle(f64),
    Rect { w: f64, h: f64 },
}

fn area(s: Shape) -> f64 {
    match s {
        Shape::Circle(r) => 3.14 * r * r,
        Shape::Rect { w, h } => w * h,
    }
}
```

`match` 支持嵌套泛型载荷解构，如 `Option<Vec<T>>`、`Result<Option<String>, i64>`。

### 5.3 trait 与泛型（单态化）

```zeta
trait Area {
    fn area(&self) -> f64;
}

impl Area for Shape {
    fn area(&self) -> f64 { /* ... */ }
}
```

泛型按单态化编译（每个具体类型实例化一份代码）。

---

## 6. 数组与索引

```zeta
let arr = [10, 20, 30];                 // 数组字面量（元素类型统一）
let arr2: [i64; 4] = [1, 2, 3, 4];      // 类型注解保留长度
let x = arr[0];                         // 索引读取（越界编译期可查）
arr[1] = 99;                            // 索引写入（别名共享，互相可见）
let ch = s[0];                          // 字符串按字符索引（步长 1 字节）
```

---

## 7. 模块系统

```zeta
mod math {
    pub const PI: f64 = 3.14159;
    pub fn square(x: i64) -> i64 { x * x }
}

use math::PI;                   // 别名导入
use math::square as sq;
```

- 多文件模块：`mod foo;` → `foo.zeta` / `foo/mod.zeta`
- 跨模块路径：`mod::Enum::Variant` / `mod::CONST`

---

## 8. 内存管理：分层所有权模型

| 层级 | 机制 | 关键字 | 运行时开销 |
|------|------|--------|------------|
| L0 | 静态所有权 + 借用 + 生命周期 | `let`, `&`, `&mut` | 零开销 |
| L1 | 区域（Region） | `region 'r {}`, `in 'r`, `transfer` | 零开销（批量释放） |
| L2 | 引用计数 | `Rc<T>`, `Arc<T>` | 原子操作 |
| L3 | 可选 GC | `Gc<T>`（插件式） | GC 停顿 |

### 8.1 L0：所有权与借用

> **MVP 状态**：L0 已实现**移动语义**（值拷贝/移动）、**方法接收者** `&self` / `&mut self`，以及**引用类型与表达式**（G1 ✅）：`&T`/`&mut T` 类型、`&x`/`&mut x` 表达式、`*` 解引用、参数与返回引用。
> 引用内存模型：标量 `&x` 存 `i8*` 值槽（`*p` 读写时按标量种类 bitcast），聚合引用拷贝对象指针；字段访问/方法调用自动剥掉引用层（`p.x` / `v.len()` 免显式解引用）。
> 借用规则：`&mut T` 可传给 `&T` 参数；严格可变性互斥 / 悬垂 / 别名检查（borrowck）已实现（见下）。
> `ref` / `ref mut` 模式已实现（见下）：`match` 臂与 `let ref x = e;` 绑定变量为对匹配值的引用而非值拷贝。
> 用户顶层函数与 std 预置根函数重名时（如自定义 `fn read` 与 std extern `read`），用户侧声明自动以 `read@shadow<N>` 内部名注册，std 模块内部裸名调用仍绑定 std 版本，用户代码绑定自身版本，互不干扰。

```zeta
let s = String::from("hello");
let t = s;               // 值拷贝：3 槽结构体复制，共享底层数据缓冲，s 仍可用
let r = &t;              // 取不可变引用（标量/聚合均可）
fn first_char_len(s: &String) -> i64 { s.len() }   // 参数引用，方法调用自动剥离引用层
let n = *r;              // 显式解引用（标量）
```

#### 严格借用检查（G1 收尾 ✅）

借用检查器（borrowck）对引用实施 NLL 近似的排他性规则：

```zeta
let mut x = 10;
let r1 = &x;              // 共享借用：多个 & 可共存
let r2 = &x;
println(*r1 + *r2);       // 20：读取被借用变量允许
let m = &mut x;           // &mut 仅在无其他活跃借用时可取
*m = 20;                  // 解引用写是"借用使用"，允许
println(x);               // 20

let q = &x;               // 借用存活到最后一次使用（NLL）
println(*q);              // q 失效于此处
x = 99;                   // 允许：q 已不再活跃

let mut z = 5;
let rz = &z;
println(*rz);             // rz 失效于此处
z = 6;                    // 允许：借用已结束
```

> **编译错误（borrowck 拒绝）**：① `&mut x` 与活跃借用互斥——`let r = &x; let m = &mut x;` 报 `BorrowConflict`（多个 `&mut` 同理）；② `&mut` 要求可变绑定——对非 `let mut` 变量取 `&mut` 报 `BorrowMutImmutable`；③ 活跃借用期间直接赋值——`let r = &x; x = 6;` 报 `BorrowConflict`（Rust E0502/E0499 对应）；④ 局部引用逃逸——`return &x` 或 `return r`（`r = &x` 局部）报 `DanglingReference`（E0597，参数来源引用允许返回）。
> **语义有意比 Rust 宽松**：读取被借用变量与经 `*p` 写入均为合法模式（裸指针别名场景），仅"直接赋值被借用变量"触发 `BorrowConflict`。借用存活范围按语句粒度近似（`born..=last_use`），跨语句的精细生命周期验证（`'a` 绑定推断）仍规划中。完整正反例见 `tests/run-pass/borrow_pass.zeta` 与 `tests/compile-fail/borrow-*.zeta`。

#### `ref` / `ref mut` 模式（G1 收尾 ✅）

`match` 臂与 `let` 绑定中，`ref x` 将 `x` 绑定为对匹配值的**引用**（`&T`）而非值拷贝，`ref mut x` 绑定 `&mut T`——只读场景免去再次拷贝，且可经解引用写：

```zeta
let x = 42;
match x { ref r => println(*r) }        // 42：r: &i64

struct Point { x: i64, y: i64 }
let p = Point { x: 10, y: 20 };
match p { ref r => println(r.x + r.y) } // 30：字段访问自动剥引用层

let o = Option::Some(99);
match o {
    Option::Some(ref v) => println(*v), // 99：枚举子模式，对字段槽取引用
    Option::None => println(0),
}

let z = 5;
let ref rz = z;                         // 语句级：let ref x = e;
println(*rz);                           // 5

let mut y = 7;
match y { ref mut r => *r = 100 }       // r: &mut i64，解引用写
```

> **MVP 注意（与 Rust 的差异）**：Zeta 的 `match` desugar 先把匹配值**拷贝**到临时槽，`ref` 绑定指向该拷贝而非原变量——`ref mut r` 的写作用于拷贝，原变量不受影响（`match y { ref mut r => *r = 100 }` 后 `y` 仍为 7）。`ref` 的价值在省去绑定时的再次拷贝；聚合类型经 `ref` 绑定为对象指针拷贝，不复制底层数据。`&` 表达式目标仍限变量（复杂目标可经模式引用）。

### 8.2 L1：区域（Region）

区域内的对象随区域退出**批量释放**（零开销），可用 `transfer` 将所有权移出：

```zeta
region 'r {
    let data = BigStruct::new() in 'r;
    process(&data);
} // 批量释放

fn create_data() -> BigStruct {
    region 'r {
        let data = BigStruct::new() in 'r;
        return transfer data out of 'r;
    }
}

// 智能分配：编译器自动推断大小
region 'r adaptive {
    for i in 0..<10000 {
        let obj = Data::new(i) in 'r;
    }
}

// 分配策略显式指定（默认 bump）：`with_size (N)` 精确预分配，
// `strategy (bump)` 指定 bump 分配策略
region 's with_size (4096) {
    let buf = BigStruct::new() in 's;
}
region 't strategy (bump) {
    let p = Point { x: 1, y: 2 } in 't;
}
```

**运行时接线（L3 ✅）**：`region` 块已从"编译期类型/借用分析"接入真实运行时——region 指令（`RegionEnter`/`AllocInRegion`/`RegionExit`/`Transfer`）调用 `zeta-region-alloc` 的 C ABI 层（`zeta_region_enter`/`zeta_region_alloc`/`zeta_region_transfer`/`zeta_region_exit`），对象 `in 'r` 时经 bump 分配器从区域批量分配（聚合对象为值镜像浅拷贝），区域退出时批量释放；`transfer x out of 'r` 调用 `zeta_region_transfer` 标记所有权移出（对象不再随区域释放）。`adaptive` 区域在 `zeta build --profile`（PGO 回灌，F2）时按画像建议注入初始容量；`with_size (N)` 精确预分配，容量不足时按 `allow_growth` 扩容（`strategy (bump)` 为默认无扩容 bump）。MVP 限制：聚合对象为值镜像（浅拷贝，不注册析构），标量/聚合引用语义与 `Box<T>` 同构。**性能（2026-08-24 ✅）**：codegen 对 `AllocInRegion` 生成内联 bump 快路径（对齐 + 越界检查 + 指针递增，零函数调用），并在规范 while 循环内做**循环级 region 状态提升**——preheader 快照 Region 头、header phi 维护 base/cursor/limit、热循环零访存（纯寄存器运算）、循环退出写回一次；单块不扩容场景实测约 2.1 倍提速，完整语义（含慢路径 cursor 回写修复）下五种 region 策略 5.33–5.56ms、单次分配约 2.8ns（详见 [memory-model.md §7.4](./memory-model.md) 与 [附录 A.4](./memory-model.md) / [A.5](./memory-model.md)）。

### 8.3 堆分配：`Box<T>` / `Rc<T>` / `Arc<T>`（K2–K3 ✅）

`Box<T>` / `Rc<T>` / `Arc<T>` 为编译器内建智能指针：构造 + `*` 解引用 + 字段/方法/索引自动剥层。

```zeta
let b = Box::new(42);                 // 标量装箱
println(*b);                          // 42（解引用 load）

struct Point { x: i64, y: i64 }
let bp = Box::new(Point { x: 10, y: 20 });
println(bp.x + bp.y);                 // 30（字段访问自动剥 Box）

let bs = Box::new(String::from("hi"));
println(bs.len());                    // 2（方法调用自动剥 Box）

let bb = Box::new(Box::new(7));
println(**bb);                        // 7（嵌套装箱）

// K3 引用计数：clone 共享、强弱计数、弱引用升级、try_unwrap
let r = Rc::new(42);
let r2 = r.clone();
println(r.strong_count());            // 2
let w = r.downgrade();
println(r.weak_count());              // 1
match w.upgrade() {
    Option::Some(rc) => println(*rc), // 42
    Option::None => println(0),
}
match r.clone().try_unwrap() {
    Result::Ok(x) => println(x),
    Result::Err(rc) => println(*rc),  // 42（强计数 > 1 走 Err 分支）
}
```

布局：`Box<T>` 栈上 1 指针槽，堆上 `slot_count(T)` 个 8 字节槽（标量 1 / struct 字段数 / enum tag+字段 / 数组长度 / 元组元素数 / 引用·Fn·内嵌 Box 1 槽）；`Rc<T>` 栈上 1 槽指向堆 `RcInner`（`T` 值区自堆首槽起 + 尾部 strong/weak 计数槽，见 memory-model.md §4 MVP 注记）；聚合 T 装箱为整槽区浅拷贝。三者均可作为函数参数与返回值类型（§13）。MVP 无自动 drop（计数只增不减，与 `Vec`/`String` 一致，显式释放语义规划中）。

### 8.4 L3

`Gc<T>`（可选 GC，K4）已实现（MVP，`zeta-gc-runtime` 保守标记-清除）：

```zeta
// gc_region 块内分配，块结束触发 GC 周期（未逃逸对象回收）
gc_region {
    let a = Gc::new(42);
    println(*a); // 42
    let b = Gc::new(7);
    println(*a + *b); // 49
}

// 逃逸对象：块返回值存活到块外（zeta_gc_escape 登记为 root）
let g = gc_region { let inner = Gc::new(100); inner };
println(*g); // 100

// 字段 / 方法 / 索引自动剥层（与 Box / Rc 同构）
let gp = gc_region { Gc::new(Point { x: 5, y: 6 }) };
println(gp.x + gp.y); // 11
let gs = gc_region { Gc::new(String::from("hello")) };
println(gs.len()); // 5

// 嵌套 gc_region：内层逃逸对象在外层块内仍活跃（存活链式提升）
let outer = gc_region {
    let o = Gc::new(1000);
    let saved = gc_region { Gc::new(2000) };
    o
};
println(*saved); // 2000
println(*outer); // 1000
```

布局与生命周期协议：`Gc<T>` 栈上 1 槽指向堆 1-槽包装（槽 0 存 `GcInner` 基址）；对象 = `slot_count(T)` 个 8 字节槽，与 `Box<T>` 完全同构（解引用/剥层零差异）。`gc_region` 块 desugar 为 `zeta_gc_region_begin()` → `zeta_gc_alloc(n)` → `zeta_gc_escape(ptr)`（块返回值登记逃逸 root）→ `zeta_gc_collect()`（标记-清除 + 存活提升）。MVP 限制：stop-the-world 保守标记-清除、单线程无锁（`SyncUnsafeCell`，全程无锁）、递归标记（深引用图可能爆栈）、块外对象永不回收（泄漏语义）、跨块逃逸对象引用图泄漏至程序结束；多线程 GC（全局锁/线程局部堆）、增量回收、write barrier 规划中（memory-model.md §5）。

---

## 9. Actor 并发模型

`actor` 是语言级构造：actor 方法消息经「kind 槽 + 3 个 i64 消息槽」传递，同一 actor 的邮箱按 FIFO 互斥处理。

### 9.1 定义与调用

```zeta
actor Counter {
    value: i64 = 0,

    pub fn increment(amount: i64) -> i64 {
        self.value += amount;
        self.value
    }
}

fn main() {
    let c = Counter::new();            // 普通 spawn（无监督）
    let r = c.increment(10).await;     // ask 同步往返
    println(r);                        // 10
    send c.increment(1);               // fire-and-forget 异步发送
}
```

- `new()`：编译器生成 `__state_new` + `zeta_actor_spawn` 调用
- 方法调用 `.await`：ask 同步往返；`send expr`：异步入队
- 消息按 FIFO 处理，`send` 后立刻 `ask` 能看到累积状态

### 9.2 监督与崩溃恢复

```zeta
actor Machine {
    uptime: i64 = 0,

    pub fn tick(n: i64) -> i64 {
        self.uptime += n;
        self.uptime
    }

    pub fn explode() -> i64 { -1 }     // 返回 -1 = 崩溃信号
}

fn main() {
    let m = Machine::new_supervised(0);  // 0=OneForOne 1=AllForOne 2=RestartForOne
    println(m.tick(5).await);            // 5
    println(m.explode().await);          // 0（ask 立即失败返回 0）
    println(m.tick(3).await);            // 3（supervisor 已重启，状态回到初始值）
}
```

崩溃协议：方法返回 `-1` → runtime 判定 Panic → ask 立即返回 0；受监督 actor 由 supervisor 经 `__state_new` 重建初始状态并重启（OneForOne 只重启崩溃者，AllForOne 重启全部，RestartForOne 逐任务重启）。

### 9.3 普通函数 `async fn` / `await`（S1c ✅ 状态机）

```zeta
// async fn 编译为 Future 结构体 + poll 状态机 + 构造器（desugar，真实挂起/恢复）
async fn get_value(x: i64) -> i64 {
    x * 2
}

// async fn 内可 .await 其它 async fn（嵌套）
async fn nested(x: i64) -> i64 {
    let v = get_value(x).await;
    v + 1
}

// 多 await 顺序执行 + 跨段变量
async fn chain(x: i64) -> i64 {
    let a = get_value(x).await;
    let b = get_value(a).await;
    a + b
}

fn main() {
    // block_on 状态机轮询
    let mut fut1 = get_value(21);
    println(block_on(&mut fut1));   // 42
    let mut fut2 = nested(10);
    println(block_on(&mut fut2));   // 21
    let mut fut3 = chain(3);
    println(block_on(&mut fut3));   // 6 + 12 = 18
}
```

- `async fn foo(args) -> i64` desugar 为 `struct __Fut_foo` + `impl Future for __Fut_foo` + 构造器 `fn foo(args) -> __Fut_foo`；`f(args)` 返回 future，`block_on(&mut fut)` 循环轮询直至 `Poll::Ready`。
- `.await` 经状态机轮询子 future：await 段 `k` 占状态 `2k`（首轮询，初始化子 future 并 poll）/ `2k+1`（恢复轮询）；`Poll::Pending` 保存状态并返回挂起，`Poll::Ready(__v)` 推进到下一段。
- 跨 await 的 `i64` 变量与子 future 提升为结构体字段（按数据依赖拓扑排序）。
- 支持形式：`let v = e.await;` / `e.await;`（语句）/ `return e.await;` / 块尾表达式 `e.await`；await 目标为 async fn 直接调用或带类型注解的变量（`let fut: G = ...; fut.await`）。
- **MVP 限制**：async fn 参数限 `i64`、返回限 `i64`/`()`；控制流块内 await 与表达式嵌套 await（`a.await + b.await`）不支持；递归 async fn 不支持。
- 与 actor 机制分工：actor 方法 `.await` = ask 同步往返（§9.1）；普通 `async fn` 为独立状态机（与 actor 互不相关）。

> **规划中**：泛型 `join_all`（Future 版）/ `timeout`（Result 版）/ `sync` 并发原语（std-lib §10）；await 位于控制流块 / 表达式中间、按引用捕获（std-lib §10.3）。

---

### 9.4 JSON 序列化 / 反序列化（L2 ✅）

`json::stringify(v)` 将值序列化为 JSON 文本；`json::parse::<T>(s)` 经 turbofish 泛型实参指定目标类型反序列化。

```zeta
struct Point { x: i64, y: i64 }

fn main() {
    // stringify：标量 / String / &str（含转义）/ 数组 / struct / Vec / HashMap
    println(json::stringify(42));                          // 42
    println(json::stringify(true));                        // true
    println(json::stringify("hi"));                        // "hi"
    println(json::stringify("a\"b"));                      // "a\"b"
    println(json::stringify([1, 2, 3]));                   // [1,2,3]

    let p = Point { x: 10, y: 20 };
    println(json::stringify(p));                           // {"x":10,"y":20}

    let v: Vec<i64> = vec![1, 2, 3];                       // 元素类型须确定（注解）
    println(json::stringify(v));                           // [1,2,3]

    let m: HashMap<i64, i64> = map![3 => 30, 1 => 10, 2 => 20];
    println(json::stringify(m));                           // {"1":10,"2":20,"3":30}（键序确定性）

    // parse（turbofish `::<T>`）：直接返回 T（MVP 宽松语义）
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

- `json::stringify(v)`：`i64`→十进制；`bool`→`true`/`false`；`String`/`&str`→带引号 JSON 字符串（`"` `\` 换行 制表 转义为 `\"` `\\` `\n` `\t`）；数组→`[e0,e1,...]`（静态展开）；struct→`{"f":v,...}`（字段序 = 定义序，嵌套递归）；`Vec<T>`→`[e0,e1,...]`（while 循环）；`HashMap<K,V>`→`{"k":v,...}`（L2f，i64/String 键，键序确定性，值递归）。
- `json::parse::<T>(s)`：turbofish 泛型实参（`::<T>`，parser 三 token 前瞻检测；嵌套泛型 `>>` 拆分层）；支持 `i64` / `bool` / `String` / `HashMap<K,V>`（L2g：i64/String 键 + 标量值 i64/bool/String）；**MVP 语义直接返回 `T`**——非法输入给默认值（`0` / `false` / 空串 / 空 map），非 Result 包装。
- MVP 限制：`map![...]` 绑定后 K/V 为 `Infer`，须 `let m: HashMap<i64, i64>` 注解（与 `vec![...]` 一致）；`HashMap` parse 的键/值含逗号或冒号时经 `split(",")` / `find(":")` 分段不可靠；嵌套 `HashMap` 值 parse 报 Unsupported（值限标量）；自定义 `Serialize` / `Deserialize` trait 与 `#[derive]` 宏规划中（std-lib §9）。

---

## 10. 标准库

### 10.1 内建打印

```zeta
println("Hello, Zeta!");   // 字符串字面量
println(42);               // 整型
println(3.14);             // 浮点
println(true);             // bool
println();                 // 空行
print(x);                  // 同 println 但不换行
```

### 10.2 String

> **实参自动升级**：`String` 形参位置传入字符串字面量（或绑定字面量的变量）自动构造 String 对象——`f("..")` / `m.push_str("!")` / `m.contains("z")` / `map.insert("k", 1)` 直接可用，无需 `String::from`。升级覆盖实例 / 静态方法、普通函数、函数指针、泛型调用与 `dyn Trait` 方法（与 IIFE / 闭包值实参的升级语义一致）；`str` 形参保持裸字面量指针直传；运行期 `str` 值实参报 Unsupported。
>
> **`&str` 只读借用视图（G2）**：`s.as_str()` 返回对 String 内容的只读借用（零拷贝，运行时为指向 String 对象的瘦指针）；`&str` 支持 `len()` / 字节索引 `r[i]` / `substring`（拷贝）/ 内容比较，也可作为函数参数（`&String` 与 `&str` 均可传入）与返回值。`String::from` 三类参数：字面量（编译期长度）、运行期 `String` 变量（`≡ s.clone()` 深拷贝）、`&str` 视图（读 data/len 槽深拷贝）。
>
> **`str` 值一等类型**：`let s = "hi"` 绑定的 `s`（运行时为指向静态字面量数据的 `i8*` 指针，无 String 的 len/cap 槽）参与字符串操作时自动**升级为 String 对象**（编译期长度展开，与 `String::from(s)` 一致）：
> - 方法调用：`s.len()` / `s.substring(0, 1)` / `s.contains(...)` / `s.as_str()` 等按 String impl 解析；
> - 拼接：`s + "!"`、`"ab" + "cd"`、`s + s`（左 / 右操作数为 Str 值 / String 均可，结果为 String）；
> - 比较：`s == "hi"` / `s < "zz"` / `s == String::from("hi")` 等（Str 值 ↔ 字面量 / String / `&str` 内容比较，字典序）。
>
> 运行期 `str` 值（如 `fn f(s: str)` 的函数参数，内容与长度运行期未知）不支持上述升级，方法调用报 Unsupported。

```zeta
let s = String::from("Hello, Zeta");
s.len                       // 字节长度（.len 字段 / .len() 方法均可）
s[0]                        // 按字节索引
s.substring(0, 5)           // "Hello"
let r = s.as_str();         // &str 只读借用视图
r.len()                     // 15
r[1]                        // 101（'e'，按字节索引）
String::from(r)             // 深拷贝 &str → 独立 String
s.contains("Zeta")                                // true（字面量实参自动升级）
s.starts_with("Hello")                            // true
s.replace("Hello", "Hi")                          // "Hi, Zeta"
s.to_upper() / s.to_lower() / s.trim()
s.split(",")                                      // Vec<String>
s.repeat(3)
s.strip_prefix("Hello")                           // Option<String>
s.truncate(5)
int_to_string(42)           // "42"
string_to_int("42")         // 42（字面量实参自动升级）
```

### 10.3 Vec

```zeta
let mut v: Vec<i64> = Vec::new();   // 建议显式类型注解（`sort` 等泛型方法需要定型）
v.push(10); v.push(20); v.push(30);
v.len / v.cap                // 字段；v.is_empty() 为方法（需括号）
v[0] / v.set(i, x)           // 索引读写
v.get(i)                     // 按索引取值（返回 T 而非 Option；越界为未定义行为）
v.pop()                      // Option<T>
v.contains(20)               // bool
v.find(20)                   // 下标，未找到 -1
v.remove(i) / v.insert(i, x)
v.sort() / v.reverse() / v.swap(i, j)
v.first() / v.last()         // Option<T>
v.binary_search(20)          // 最左下标，未找到 -1
v.clear()
for x in v { }               // 容器迭代（仅 Vec / HashMap）
```

### 10.4 HashMap

```zeta
let mut m: HashMap<String, i64> = HashMap::new();
m.insert("k", 42);                  // 键字面量实参自动升级为 String
m.contains_key("k")                 // bool
m.get("k")                          // Option<V>
m.remove("k")                       // Option<V>
m.len / m.cap               // 字段；m.is_empty() 为方法（需括号）
m.clear()
for (k, v) in m { }         // 容器迭代
```

### 10.5 Option / Result

```zeta
let x = Some(10);
x.is_some() / x.is_none()
x.unwrap()                  // 10
x.unwrap_or(0) / x.expect("msg")

let r: Result<i64, i64> = Ok(7);
r.is_ok() / r.is_err()
r.unwrap() / r.unwrap_or(0)
```

### 10.6 文件 IO

```zeta
let r = read_file("data.txt");                       // Result<String, IoError>：读整个文件
match r {
    Ok(content) => { let _ = write_file("out.txt", content); }  // Result<i64, IoError>：截断写
    Err(e) => println(e.message()),                  // 打开失败：错误消息（M3 起 Result 化）
}
let _ = append_file("log.txt", line);                // Result<i64, IoError>：追加
let line = read_line();                              // Result<String, IoError>：stdin 读一行（不含换行符）
```

### 10.7 网络

```zeta
let host = hostname();                     // Result<String, IoError>：本机主机名（M3 起 Result 化）
let pair = socketpair_stream();            // AF_UNIX SOCK_STREAM 全双工 fd 对
let _ = send_all(fd, data);                // Result<i64, IoError>：发送字节数（data 为 String 变量）
let got = recv_some(fd, n);                // Result<String, IoError>：接收（SOCK_STREAM 需循环收满）
let fd = tcp_connect(8080, 127, 0, 0, 1);  // Result<i64, IoError>：TCP 连接（port, a, b, c, d）
```

### 10.8 同步原语

```zeta
let mutex = Mutex::new();
mutex.lock();
// 临界区
mutex.unlock();
mutex.try_lock()            // 已锁返回 0（EBUSY），成功返回非 0

let rw = RwLock::new();
rw.read_lock();  rw.unlock();
rw.write_lock(); rw.unlock();
rw.try_read_lock() / rw.try_write_lock()
```

### 10.9 时间

```zeta
let d = Duration { micros: 1500000 };
d.secs()        // 1（向下取整）
d.millis()      // 1500
d.micros()      // 1500000
d.nanos()       // 1500000000

let t = Instant::now();
// ... 执行一段计算 ...
let elapsed = t.elapsed();   // Duration，CPU 时钟差，恒 >= 0
```

### 10.10 位运算

`&` `|` `^` `<<` `>>` 全链路支持（常量折叠 + 运行时两路径）：

```zeta
let port = (0xC0A8 << 8) | 0x010A;   // 字节打包
let byte = (port >> 8) & 0xFF;       // 解包取高字节
```

> 注意优先级：`*` > `+` > `<<` > `&` > `^` > `|`，且比较 `>` 位运算，裸 `x & 3 == 2` 会按 bool 处理，需要括号 `(x & 3) == 2`。

### 10.11 动态切片（数组 / Vec）

`v[lo..<hi]`（半开）、`v[lo...hi]`（双闭）、`v[lo<..hi]`（不含下界）返回**全新缓冲**（元素按值拷贝），越界自动 clamp，`start >= end` 返回空：

```zeta
let arr = [10, 20, 30, 40, 50];
let a = arr[1..<3];              // [20, 30]
let b = arr[lo..<hi];            // 动态边界（运行时变量）

let mut v: Vec<i64> = Vec::new();
v.push(1); v.push(2); v.push(3); v.push(4);
let sub = v[1..<3];              // [2, 3]；原 Vec 不受影响
let c = v[-3..<2];               // clamp 到 [0, 2) → [1, 2]
let d = v[4..<1];                // start >= end → 空
```

字符串切片 `s[lo..<hi]` 同理（按字节）。

---

## 11. 编译目标与工具链

### 11.1 本机编译

```bash
zeta build hello.zeta -o hello
./hello
```

### 11.2 交叉编译

```bash
# macOS 双架构（已验证）
zeta build app.zeta --target x86_64-apple-macosx
zeta build app.zeta --target arm64-apple-macosx
```

目标平台通过 `__zeta_target_os` 平台内建区分（linux=1 / macos=2 / windows=3 / freebsd=4 / 其他=0），标准库据此选择平台布局（如 `sockaddr_in4` 的 macOS `sin_len` 头 vs Linux 无该字段）。

### 11.3 WebAssembly（E2）

```bash
zeta build app.zeta --target wasm32-wasip1 -o app.wasm
wasmtime app.wasm      # WASI preview1 运行时运行
```

依赖：`wasi-libc` sysroot（`brew install wasi-libc` 或 `WASI_SYSROOT`）、`wasm-ld`（`brew install lld`）。入口链：WASI `_start`（crt1）→ Zeta `main()`。未使用的 extern 符号（socket 等 WASI 无对应）不会引入链接错误。

**Actor 交叉编译**（L4 ✅）：`--target wasm32-wasip1` 下 actor 程序同样受支持——driver 探测并链接 wasm 版 `zeta-actor-runtime`（`cargo build --target wasm32-wasip1 -p zeta-actor-runtime`；缺失且源码用了 actor 时显式报错）；dlsym 在 WASI 不可用，driver 从 HIR 收集 actor 符号注入静态 `zeta_actor_resolve` 符号表（strcmp → 函数地址），运行时走 WASI 单线程同步模式（同 native 邮箱 FIFO 互斥 + 受监督崩溃重启协议）。网络（`net` 模块）在 WASI 下**明确禁用**（`__zeta_target_os` 码 5 短路返回，见 §13 约束 2）。

---

## 12. 外部函数接口（FFI）

`extern fn` 声明无 body 的 libc 符号，由链接器解析（通用 FFI，阶段 A4 打通）：

```zeta
extern fn clock() -> i64;
extern fn fopen(path: String, mode: String) -> i64;
extern fn gethostname(name: String, len: i64) -> i64;
```

支持的标量类型：`i8/i16/i32/i64/isize/u8/u16/u32/u64/usize/f32/f64/bool/char/()/&T/String`。
`String` 参数在 ABI 层传递 data 指针；`i32` 返回值自动 `sext` 清洗，规避高位未定义。

---

## 13. 参考与已知限制

### 权威文档

| 文档 | 内容 |
|------|------|
| [CODEBUDDY.md](../CODEBUDDY.md) | 项目总纲（架构、工具链、执行记录） |
| [grammar.md](./grammar.md) | 完整语法规范（EBNF） |
| [semantics.md](./semantics.md) | 语义规则 |
| [memory-model.md](./memory-model.md) | 分层内存管理规范 |
| [actor-model.md](./actor-model.md) | Actor 并发模型规范 |
| [std-lib.md](./std-lib.md) | 标准库 API 规范（含规划中模块） |
| [development-plan.md](./development-plan.md) | 开发计划（阶段 A–F，权威执行记录） |
| [mvp-gaps-plan.md](./mvp-gaps-plan.md) | 已知限制消解计划（阶段 G–L，承接 A–F 后） |

### 已知限制（MVP）

**规划中 / 未实现**：
- **宏系统**：已实现 `macro_rules!` 声明式宏（`$x:expr`/`$x:ident`/`$x:ty`/`$x:tt` + `$(`...`)` 重复 `*`/`+`/`?`，parse 期 AST 展开）：`$x:expr` 捕获**完整表达式 token 序列**（token 级优先级爬升：前缀一元 `-`/`!`/`not`/`&`/`*`、二元中缀、后缀调用/索引/成员/`?`/`as` 转换，`a > b`、`-1`、`f(x) + 1`、`s.len()`、嵌套宏调用 `m!(x)` 均可；按原样展开，优先级由调用方负责，与 Rust 语义一致）+ 内置格式化宏 `println!` / `print!` / `format!` / `dbg!` + `eprintln!` / `eprint!`（N4 ✅，输出 stderr，经 POSIX `dprintf(2, ...)` 直写；`{}` 占位、`{:?}` 同构、`{{`/`}}` 转义 + 多参数可变长度，typecheck desugar 为 String 拼接 + 内建打印）+ 集合宏 `arr!` / `vec!` / `map!`（I3 ✅，见 §3.2）：`arr![a, b]` parse 期产出数组字面量；`vec![a, b]` / `map![k => v]` desugar 为块表达式（`Vec::with_capacity(n)` / `HashMap::with_capacity(n)` + 逐元素 `push` / `insert`，空集合 → `new()`），元素支持任意表达式与嵌套宏调用。`r#"..."#` 带哈希原始字符串已实现（lexer 支持 `r#`/`r##` 等任意哈希定界、无转义，与 `r#ident` 原始标识符区分；`println!(r#"..."#)` 可用）。限制：无卫生宏（hygiene，全局名称匹配）。
- **引用与借用**：`&x`/`&mut x` 表达式、`&T`/`&mut T` 参数类型、解引用 `*`、返回引用均已实现（G1 ✅，见 §8.1）；`&str` 只读借用视图已实现（G2 ✅：`String::as_str()` + `&str` 参数/返回/索引 + `String::from(&str)` 深拷贝，见 §10.2）；`str` 值（字符串字面量 / 绑定字面量的变量）已实现一等类型语义（方法调用 / `+` 拼接 / 内容比较自动升级为 String 对象；**String 形参位置的字面量实参自动升级**——普通 / 泛型函数、实例 / 静态方法、函数指针、`dyn Trait` 方法调用均可直接传字面量，见 §10.2）；裸指针已实现（G3 ✅：`*const T`/`*mut T` 类型 + `*p` 读写 + `&T`↔`*const T` 互视 + `*mut` 降级 `*const`，见 §8.2）；生命周期标注已实现（G4 ✅ MVP 语法接受：`<'a>` 与 `&'a T` 解析后丢弃，宽松检查）；`ref` / `ref mut` 模式已实现（G1 收尾：`match` 臂与 `let ref x = e;` 绑定变量为对匹配值的引用而非值拷贝，枚举子模式 / struct 字段 / 解引用写均可用，见 §8.1；MVP 注意——`match` 先拷贝匹配值，`ref` 绑定指向拷贝，与原变量无关）；**严格借用检查已实现**（G1 收尾，见 §8.1）：NLL 近似的借用排他性——`&mut` 与任何活跃借用互斥、多个 `&mut` 互斥、活跃可变借用期间写入被借用变量报 `BorrowConflict`（E0502/E0499）；`&mut` 要求 `let mut` 绑定报 `BorrowMutImmutable`（E0596）；局部引用逃逸函数（尾表达式 / `return` 返回 `&x` 或绑定引用变量）报 `DanglingReference`（E0597，参数来源引用允许返回）；语义有意宽松——读取被借用变量与经 `*p` 写入允许（裸指针别名合法），共享借用（多个 `&`）可共存，仅直接赋值被借用变量触发冲突；`print`/`println` 参数为引用时自动剥层打印解引用值（`println(r)` ≡ `println(*r)`）；严格生命周期验证仍规划中。
- **闭包**：✅ 无捕获闭包已实现（H2，见 §3.2）：`|x, y| expr` desugar 为匿名函数 + 函数指针（零运行时开销），需 fn 类型上下文（fn 形参实参 / `let f: fn(..) = |..| ..` 注解绑定）驱动参数类型推断；参数模式仅支持简单标识符与 `_`；**返回闭包的函数已实现**（`fn make() -> fn(..) { |x| .. }` 尾闭包按 H2 签名检查生成函数指针）。捕获闭包已实现（H3 IIFE MVP，见 §3.2）：`(|x| body)(args)` 立即调用按值捕获（desugar 为匿名函数 + 捕获变量前置调用）。闭包值对象已实现（H5 补全，见 §3.2）：`let f = |x: i64| ..; f(..)` 绑定后反复调用（按值捕获，desugar 为捕获聚合对象 + 调用点字段读取展开；仅局部变量环境）；**参数类型规则**——有注解用注解、无注解由首次调用点实参推断（`let f = |x| x + 1; f(41);`，半注解亦可用，惰性检查）；**无捕获闭包值可作 fn 实参/返回值**（降级为函数指针 / 按 fn 签名固化）；捕获闭包值不跨函数边界；按引用捕获、`move` 所有权语义规划中。
- **函数指针**：✅ 已实现（H1，见 §3.2）：`fn(T) -> R` 类型 + `let f = add` 函数值绑定 + `f(args)` 间接调用；函数值可作实参、返回值、重新绑定、类型注解。
- **运算符**：✅ `?` 错误传播已实现（K1，见 §3.2）：`expr?` 在 Option/Result 上下文 desugar 为 `match` + `return` 早返回（`Some(__v) => __v` / `None => return Option::None`，Result 为 `Err(__e) => return Result::Err(__e)`）；支持表达式中间嵌套 `?`；裸无参变体值表达式（`return None;`）可用；`?` 用于非 Option/Result 类型报错。`dyn Trait` ✅ 已实现（H4，见 §3.2）：trait 对象（`dyn Trait` 类型 + `&T` 强制转换 + vtable 间接分派）；MVP 限制：trait/impl 非泛型、含 `Self` 签名方法不可经 dyn 调用。
- **所有权层级**：✅ `Box<T>`（K2）与 `Rc<T>` / `Arc<T>`（K3）已实现（见 §8.3）：`Box::new` 堆分配 + `*` 解引用 + 字段/方法/索引自动剥层，嵌套装箱与赋值指针共享可用；`Rc`/`Arc` 支持 `clone`（强计数 +1 共享）、`strong_count`/`weak_count`、`downgrade`→`Weak`、`Weak::upgrade`、`try_unwrap`（`Result<T, Rc<T>>`），与 `Box` 同构剥层。无自动 drop（计数只增不减，与 `Vec`/`String` 一致）。L3 `Gc<T>`（K4）✅ 已实现（MVP，见 §8.4）：`Gc::new` 编译器内建 + `gc_region` 块（desugar 为 `zeta_gc_region_begin`/`zeta_gc_alloc`/`zeta_gc_escape`/`zeta_gc_collect`）+ 逃逸对象 root 登记 + 嵌套块存活链式提升 + 字段/方法/索引自动剥层；保守标记-清除运行时（`zeta-gc-runtime`，纯 `libc::malloc`/`free` 链表元数据，单线程无锁）。MVP 限制：块外对象永不回收（泄漏语义）、跨块逃逸对象引用图泄漏至程序结束、stop-the-world 非增量、递归标记；多线程/增量/write barrier 规划中。
- **并发**：`fmt` 模块为规划；actor 的 `async` 方法 + `.await` + `send` 已实现（见 §9）；普通函数 `async fn` / `.await` 已支持（S1c ✅，见 §9.3：`async fn` desugar 为 Future 结构体 + poll 状态机 + 构造器，`block_on` 轮询驱动，支持 `Poll::Pending` 挂起与恢复）；`json` 序列化已实现（L2 ✅，见 §9.4：`json::stringify` / `json::parse::<T>`，turbofish 泛型实参；标量 / 数组 / struct / Vec / `HashMap` 序列化 + `i64` / `bool` / `String` / `HashMap` 反序列化；`Serialize` / `Deserialize` trait 与 `#[derive]` 宏规划）。
- **迭代器协议**：✅ J1–J3 已实现（见 §3.2 迭代器与适配器小节）：`for i in 0..<10` 数值区间、`for x in vec` / `for (k, v) in map` / `for x in arr`（数组迭代）容器迭代可用；自定义迭代器（`next() -> Option<T>` 方法）接入 `for`；适配器 `map`/`filter`/`fold`/`collect`/`take`/`skip` 可用（返回 `Vec<T>` 可链式）。`Iterator` trait 定义（std-lib §2.3）仍为规划 API（适配器为编译器内建 desugar，非 trait 实现）。

**实现约束**：
- **std 模块化**：标准库位于 `zeta-std/zeta/`，`core.zeta` 根模块（String / Vec / HashMap / Option / Result + extern 集中声明）拆分为 `time` / `io` / `net` / `sync` 四个子模块文件，driver 加载时模块展开 + `use` 重新导出，用户侧裸名即用。
- **net**：`tcp_connect` 依赖平台特定 `sockaddr_in4` 布局（已由 `__zeta_target_os` 双布局化）；WASI 下网络不可用，**明确禁用**（L4 ✅：`net.zeta` 网络函数在 `__zeta_target_os() == 5`（WASI）时短路返回，不会链接 socket 符号；其余平台行为不变）。
- **Actor 运行时**：✅ 交叉编译 / WASM 目标下 actor 程序已支持（L4，§11.3：driver 注入静态 `zeta_actor_resolve` 符号表替代 dlsym + WASI 单线程同步运行时；需先 `cargo build --target wasm32-wasip1 -p zeta-actor-runtime`）。
- **`String::from(s)`**：✅ 支持字符串字面量（及绑定字面量的变量）、运行期 `String` 变量（desugar 为 `s.clone()` 深拷贝）与 `&str` 视图（读 data/len 槽深拷贝）；G2 已消除"非字面量长度表达未实现"。
- **region 选项**：✅ `adaptive` / `with_size (N)` / `strategy (bump)` 已实现（L3，§8.2：region 指令接线 `zeta-region-alloc` C ABI 运行时 + PGO 画像回灌 `adaptive` 初始容量）；`strategy (pool)` 等规划中。
