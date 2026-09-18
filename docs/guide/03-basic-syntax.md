# 3. 基础语法

## 3.1 变量与类型

`let` 声明不可变绑定；标量类型：`i8/i16/i32/i64/isize`、`u8/u16/u32/u64/usize`、`f32/f64`、`bool`、`char`、`()`。

```rlyeh
let x = 6;                       // 类型推断：i64
let y: f64 = 3.14;               // 显式类型注解
let mut v = Vec::new();          // 可变绑定（对象修改用）
```

**块级遮蔽**（U1 ✅）：块内 `let x` 覆盖外层同名变量，块内引用指向新绑定，块结束后原变量恢复；函数/闭包体为隔离边界（看不到外层函数的局部变量），块 / 循环体 / `match` 臂可穿透所属函数层。

```rlyeh
let x = 10;
{
    let x = 20;
    println(x);                  // 20（块内遮蔽）
};
println(x);                      // 10（块后恢复）
let i = 100;
for i in 0..<3 {
    println(i);                  // 0 1 2（for 变量遮蔽外层）
}
println(i);                      // 100（循环后恢复）
```

## 3.2 函数

```rlyeh
fn add(a: i64, b: i64) -> i64 {
    a + b                        // 末表达式为返回值（隐式 return）
}

fn main() {
    let sum = add(6, 7);         // 13
}
```

### 函数一等值（函数指针，H1）

函数可以像值一样绑定、传递与调用：

```rlyeh
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

#### 无捕获闭包（H2）

闭包 `|x, y| 表达式` desugar 为匿名函数 + 函数指针（零运行时开销），参数类型由 fn 上下文推断：

```rlyeh
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

#### 捕获闭包（H3，IIFE MVP）

捕获闭包 `(|x, y| 表达式)(实参)` 以**立即调用**形式使用：闭包体引用的外层变量**按值捕获**，desugar 为匿名函数（捕获变量作前置参数、参数名保留原名）+ 普通函数调用，零新增 IR 节点：

```rlyeh
fn main() {
    // 单捕获：闭包体引用外层变量 factor
    let factor = 3;
    let r1 = (|x| x * factor)(14);              // 42

    // 多捕获 + 字符串拼接（String 语义）
    let name = String::from("rlyeh");
    let r2 = (|s| s + name)(String::from("hi "));  // hi rlyeh

    // 参数遮蔽捕获（同名参数优先）
    let v = 100;
    let r3 = (|v| v + 1)(7);                    // 8

    // 捕获数组变量做索引
    let base = [1, 2, 3];
    let r4 = (|i| base[i] * 2)(1);              // 4
}
```

MVP 约束（见 §13）：支持立即调用形式、闭包值对象（见 H5）与返回闭包的函数（H5 补全）；按值捕获（`move` 关键字 MVP 忽略，所有权宽松）；IIFE 闭包参数无类型注解（类型从实参推断）；不支持嵌套捕获闭包（外层闭包体内直接写 IIFE 捕获内层可运行）。

#### 闭包值对象（H5）

`let f = |x: i64| 表达式;` 将闭包绑定为**值对象**，之后可反复调用 `f(实参)`：闭包体引用的外层变量**按值捕获**，desugar 为匿名函数 + 捕获聚合对象（每捕获一槽）+ 调用点展开（从聚合对象读取捕获字段传给匿名函数）。**参数类型确定规则**（H5 补全）：有类型注解的参数用注解类型（调用点实参须兼容），无注解的参数由**首次调用点实参推断**（延迟固化；`let f = |x| x + 1; f(41);` 直接可用，半注解 `|x: i64, y|` 亦可用），从未被调用则闭包体不检查（惰性）：

```rlyeh
fn main() {
    let inc = |x: i64| x + 1;
    let r1 = inc(41);                        // 42

    let base = 40;
    let add_base = |x: i64| x + base;
    let r2 = add_base(2);                    // 42

    let a = 30;
    let b = 12;
    let f = |x: i64, y: i64| x + y + a + b;
    let g = f;
    let r3 = g(0, 0);                        // 42

    let s = String::from("hello");
    let f9 = |x: i64| x + s.len();
    let r4 = f9(37);                         // 42

    let f10 = |s: String| s.len() + 37;
    let r5 = f10("hello");                   // 42

    let f12 = |x: i64| { let y = x * 2; y };
    let r6 = f12(21);                        // 42
}
```

H5 补全——非注解/半注解绑定（参数由首次调用点实参推断）、返回闭包的函数、闭包值作 fn 实参：

```rlyeh
fn apply(f: fn(i64) -> i64, x: i64) -> i64 { f(x) }

// 返回闭包的函数：尾表达式为无捕获闭包 → 按 H2 签名检查生成函数指针
fn make() -> fn(i64) -> i64 { |x| x + 1 }

fn make2() -> fn(i64) -> i64 {
    let f = |x: i64| x * 2;
    f
}

fn make3() -> fn(i64) -> i64 {
    let f = |x| x * 3;
    f
}

fn main() {
    let f = |x| x + 1;
    println(f(41));                          // 42
    println(f(1));                           // 2

    let g = |x: i64, y| x + y;
    println(g(20, 22));                      // 42

    let base = 40;
    let h = |x| x + base;
    println(h(2));                           // 42

    let c = |x| x * 3;
    println(apply(c, 14));                   // 42

    let m = make();
    println(apply(m, 41));                   // 42
    let m3 = make3();
    println(apply(m3, 14));                  // 42
}
```

MVP 约束（见 §13）：参数有注解用注解、无注解由首次调用点实参推断（半注解亦可用，从未调用则惰性不检查；注解与推断/签名冲突报类型不匹配）；仅按值捕获；**无捕获闭包值可作 fn 实参/返回值**（降级为函数指针或按 fn 签名固化参数类型）；捕获闭包值不跨函数边界传递（作 fn 实参/返回值报 Unsupported/类型不匹配）；不支持嵌套捕获闭包。

#### 协议对象（H4，`dyn Protocol`）

`dyn Shape` 是协议对象类型：**2 槽胖指针**（数据指针 + vtable 指针），`&T`（具体类型引用）可强制转换为 `dyn Shape`——desugar 为运行时构造 vtable（drop/size/align 槽 MVP 置 0 + 方法表）+ 胖指针。方法调用经 vtable 间接分派，同一签名可分派到不同 impl：

```rlyeh
protocol Shape {
    fn area(&self) -> f64;
    fn sides(&self) -> i64;
}

struct Circle { radius: f64 }
impl Circle: Shape {
    fn area(&self) -> f64 { 3.14 * self.radius * self.radius }
    fn sides(&self) -> i64 { 0 }
}

struct Rect { w: f64, h: f64 }
impl Rect: Shape {
    fn area(&self) -> f64 { self.w * self.h }
    fn sides(&self) -> i64 { 4 }
}

fn main() {
    let c = Circle { radius: 2.0 };
    let r = Rect { w: 3.0, h: 4.0 };

    let d1: dyn Shape = &c;          // &T → dyn 协议 强制转换
    let d2: dyn Shape = &r;
    println(d1.area());              // 12.56（vtable 分派到 Circle::area）
    println(d2.area());              // 12.0（分派到 Rect::area）
    println(d1.sides());             // 0
    println(d2.sides());             // 4

    let d3 = d1;                     // 胖指针拷贝共享同一 vtable
    println(d3.area());              // 12.56
}
```

MVP 约束（见 §13）：protocol 与 impl 均须非泛型；方法签名含 `Self`（关联返回类型 / 参数）不支持经 dyn 调用；vtable 的 drop/size/align 槽置 0（显式释放语义与 `Box`/`Rc` 一致）。

**去虚拟化（2026-08-24 ✅）**：`let d: dyn Trait = &obj;` 绑定变量时记录来源具体类型，后续 `d.method()` 静态分派到具体类型实现（经 `instantiate_impl_method` 取 mono 符号，含模块前缀 / 泛型实例化），LLVM 可内联 / 常量折叠——`dyn_dispatch` 基准 15.5ms → 3.6ms，与 Rust（rustc -O 去虚拟化）持平（1.08x）。保守回退：dyn 变量被重新赋值（`d = ...`）时映射失效，自动回退 vtable 间接调用，多态语义不变；仅局部 `let` 绑定变量适用（H4 本就限定 dyn 仅局部变量）。

#### `?` 错误传播运算符（K1）

`expr?` 在 `Option<T>` / `Result<T, E>` 上下文解包成功值，失败时从当前函数早返回失败值：

```rlyeh
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

#### `as` 数值转换（U6）

`expr as T` 在可转换标量间做类型转换，数值语义与 Rust `as` 一致（向零截断 / 环绕截断）：

```rlyeh
let a = 3.7 as i64;          // 3：浮点 → 整数（fptosi 向零截断）
let b = (-3.7) as i64;       // -3
let c = 5 as f64;            // 5.000000：整数 → 浮点（sitofp）
let d = 300 as i8;           // 44：整数截断（trunc，环绕）
let e = (-300) as i8;        // -44
let f = 300 as u8;           // 44
let g = (-1) as u8;          // 255：负 → 无符号环绕
let h: i8 = -1;
let i = h as i64;            // -1：符号扩展（sext）
let j = 5 as bool;           // true：整数 → bool（非零即真）
let k = 0 as bool;           // false
let l = true as i64;         // 1：bool → 整数（zext）
let m = 'a' as i64;          // 97：char → 整数
let n = 66 as char;          // 'B'：整数 → char
let o = 42 as i64;           // 42：同类型零指令（恒等透传）
```

- 可转换标量：≤64 位整族（i8/u8/…/i64/u64）+ 浮点（f64/f32）+ bool + char
- 语义映射：浮点↔整数 `fptosi`/`fptoui`/`sitofp`/`uitofp`；整数截断/扩展 `trunc`/`sext`/`zext`；整↔bool `icmp ne 0`/`zext i1`；整↔char `trunc`/`zext`；同存储恒等零指令
- 窄化整数转换后按符号恢复 64 位槽表示（符号扩展不变量），`let a: i8 = -1; a as i64` 得 -1
- MVP 限制：i128/u128（存储非 64 位槽）、指针/引用/聚合（struct/数组）转换保持擦除/不支持
- 落地价值：解锁 `time::Duration::from_secs_f64`（`(secs * 1e6) as i64`，见 §9 时间模块）

#### 迭代器与适配器（J1–J3）

```rlyeh
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
- MVP 约束：适配器参数必须是无捕获闭包（捕获外部变量报错，H3 规划）；适配器返回 `Vec<T>`（急切求值），结果 Vec 可作下一适配器源（链式）；`Iterator` protocol 定义（std-lib §2.3）为规划 API（适配器为编译器内建）

## 3.3 控制流

```rlyeh
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

`for` 支持数值区间（步长固定为 1）与 Vec / HashMap / 数组容器迭代：`for x in vec`（见 §10.3）、`for (k, v) in map`（见 §10.4）、`for x in arr`（数组迭代，见 §6）。`break` / `continue` 均可用。`match` 用于枚举解构（见 §5.2）。

## 3.4 注释

```rlyeh
// 行注释
/// 文档注释（`rlyeh doc` 提取生成 Markdown）
```

---

## 练习

1. 运行 [`examples/by-chapter/03-basic-syntax.rl`](../../examples/by-chapter/03-basic-syntax.rl)，观察变量遮蔽（`let x = 10 { let x = 20 }`）的输出顺序。
2. 写一个 `factorial(n: i64) -> i64` 函数，体会"末表达式即返回值"（不必写 `return`）。
3. 用无捕获闭包 `apply(|a, b| a * b, 6, 7)` 替代普通函数指针调用，理解 H2 闭包的零开销本质。

---

[← 上一章：快速上手](./02-quick-start.md) | [返回指南目录](./index.md) | [下一章：数学式条件判断 →](./04-math-conditions.md)
