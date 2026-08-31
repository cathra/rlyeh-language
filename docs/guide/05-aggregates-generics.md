# 5. 聚合类型与泛型

> 本章目标：掌握 Rlyeh 的**结构体（struct）**、**枚举（enum）与模式匹配（match）**、**trait 与泛型**、**类型联合**和**显式判别式**。这些是你从 C 的 `struct`/`enum` 进阶到"带行为的复合类型"的关键。
>
> 读完你能回答：Rlyeh 的 `struct` 和 C 的 `struct` 有何不同？`enum` + `match` 比 C 的 `enum` + `switch` 强在哪？泛型怎么用？

---

## 5.1 结构体 `struct`

结构体把多个字段聚合成一个值。声明和构造：

```rlyeh
struct Point {
    x: f64,
    y: f64,
}

let p = Point { x: 1, y: 2 };
println(p.x);                      // 1：用点号访问字段，和 C 一样
p.y = 3;                           // 若 p 是 let mut，可改字段
```

> **C 程序员的视角**：Rlyeh 的 `struct` 和 C 的 `struct` 几乎一模一样——**默认按值语义**（赋值/传参时整体拷贝），字段用 `.` 访问。区别：
> - 字段声明是 `名字: 类型`（类型在冒号后），而 C 是 `类型 名字;`。
> - Rlyeh 的 `struct` 可以有**方法**（见 §5.3 `impl`），C 不行（C 要靠独立函数 + 传指针）。
> - 默认不可变（见 [教程 §1.5](../tutorial/01-what-is-rlyeh.md)）：要修改字段，变量须用 `let mut` 声明。

### 结构体字面量构造

构造时必须写 `字段名: 值`（具名字段），不像 C 可以省略字段名：

```rlyeh
let p = Point { x: 1, y: 2 };   // ✅ 必须带字段名
// let p = Point { 1, 2 };      // ❌ Rlyeh 不支持位置构造（避免字段顺序错乱）
```

> **为什么要求字段名**：字段顺序在重构时容易动，具名构造让代码在"加字段/调顺序"后依然可读、不易出错（类似 Go 的结构体字面量，而非 C 的聚合初始化）。

---

## 5.2 枚举与模式匹配 `match`

C 的 `enum` 只是一组**整数常量**（没有关联数据）。Rlyeh 的 `enum` 强大得多：每个变体可以**携带数据**，并用 `match` 安全地解构。

```rlyeh
enum Shape {
    Circle(f64),              // 元组式：携带一个半径
    Rect { w: f64, h: f64 },  // 结构体式：携带宽高
}

fn area(s: Shape) -> f64 {
    match s {
        Shape::Circle(r) => 3.14 * r * r,        // 把半径绑定到 r
        Shape::Rect { w, h } => w * h,           // 把宽高绑定到 w, h
    }
}
```

> **C 程序员的视角**：这等价于 C 里"手动用 tag + union + switch"的惯用法，但 Rlyeh **替你保证了穷尽性**——如果 `match` 漏了某个变体，编译器报错（C 的 `switch` 漏 `case` 只是警告甚至不警告，运行时才可能出事）。

### 具名变体 + 模式匹配

变体可以没有数据（`Quit`）、带元组数据（`Write(String)`）、带结构体数据（`Move { x, y }`）：

```rlyeh
enum Message {
    Quit,
    Move { x: i64, y: i64 },
    Write(String),
}

fn handle(msg: Message) {
    match msg {
        Message::Quit => println(0),
        Message::Move { x, y } => println(x + y),
        Message::Write(s) => println(s.len()),
    }
}
```

模式支持：枚举变体 `Enum::Variant(...)`、结构字段 `{ w, h }`、嵌套（如 `Option<Vec<T>>`）均支持。

> **和 C 的 `switch` 最大区别**：`match` 是**表达式**（有值），所以可以像 `area = match s { ... }` 这样直接赋值；同时它**强制覆盖所有变体**，编译器在编译期检查"有没有漏掉哪种情况"。

### 用 `match` 处理"可能失败"：Option 与 Result

Rlyeh 用枚举表达"可能有值 / 可能出错"，避免 C 里 `NULL` / 特殊返回值带来的隐患：

```rlyeh
// Option<T>：有值(Some) 或 无(None)
let maybe: Option<i64> = Some(5);
match maybe {
    Some(v) => println(v),
    None => println("没有值"),
}

// Result<T, E>：成功(Ok) 或 失败(Err)
let res: Result<i64, String> = Ok(10);
match res {
    Ok(v) => println(v),
    Err(e) => println(e),
}
```

> **C 程序员的视角**：C 里函数失败常返回 `-1` 或 `NULL`，调用方容易忘记检查。Rlyeh 把这个"可能为空/可能失败"编码进**类型**里——你不 `match` 处理 `None`/`Err`，编译器就不让你用里面的值。这是"把错误当返回值显式传递"的安全版。更省事的写法见 [§3.4 的 `?` 运算符](../guide/03-basic-syntax.md)。

---

## 5.3 trait 与泛型（单态化）

### trait：定义"能做什么"的接口

`trait` 类似 C++ 的纯虚类 / Java 的 interface / C 的"函数指针结构体"，描述一组方法：

```rlyeh
trait Area {
    fn area(&self) -> f64;
}

impl Area for Shape {
    fn area(&self) -> f64 {
        match self {
            Shape::Circle(r) => 3.14 * r * r,
            Shape::Rect { w, h } => w * h,
        }
    }
}
```

> **C 程序员的视角**：C 里要实现"多种形状都能算面积"，通常要手写一个 `struct ShapeVtable { area_fp area; }` 然后每个类型挂一个 vtable——这正是 Rlyeh `trait` + `impl` 自动替你做的事。你只写"谁实现了什么方法"，调用时编译器（默认）在编译期决定调哪个版本（单态化，零运行时开销）。

### 泛型：写一个适用于多种类型的函数/类型

```rlyeh
struct Wrapper<T> {
    value: T,
}

impl<T> Wrapper<T> {
    fn new(v: T) -> Wrapper<T> {
        Wrapper { value: v }
    }
}
```

`T` 是类型参数，`impl<T>` 表示"为任意 T 实现这套方法"。调用 `Wrapper::new(5)` 时，`T` 被推断为 `i64`。

> **单态化（monomorphization）**：Rlyeh 的泛型在编译期为用到的每个具体类型**生成一份专用代码**（和 C++ 模板同理）。所以 `Wrapper<i64>` 和 `Wrapper<String>` 是两份不同的机器码，调用零开销——但编译产物会稍大（模板膨胀）。

### 泛型 trait 约束（bound）

可以为特定类型实现特定泛型 trait：

```rlyeh
trait Convert<T> {
    fn convert(&self) -> T;
}

impl Convert<i64> for f64 {
    fn convert(&self) -> i64 {
        *self as i64          // f64 → i64（向零截断）
    }
}

let n = 3.7.convert();         // n: i64 = 3
```

> **MVP 限制**：`Self` 仅支持出现在**返回位置**（如 `fn from(v) -> Self`）；出现在参数位置的关联类型暂禁。含 `Self` 签名的方法不能经 `dyn Trait` 调用（见 [§3.4 dyn Trait](../guide/03-basic-syntax.md)）。

---

## 5.4 类型联合 `A | B`（U1 / U2 ✅）

类型联合 `A | B` 表示"值是 A **或** B"，写起来比 `enum` 更轻量，运行时是匿名 enum（槽 0 = 判别标签 tag，槽 1 = 数据 payload）：

```rlyeh
let x: i64 | String = 5;            // 直接用成员值构造（协变）
match x {
    i64 => println(i64),            // 类型臂：payload 绑定到同名变量 i64
    String => println(String.len()),
}
```

> **C 程序员的视角**：这等价于 C 的 `union` + 一个手工 tag 字段，但 Rlyeh 帮你维护 tag 与 payload 的一致性，并在 `match` 时强制你处理每种类型。
>
> **未收窄的联合不能直接运算**：在 `match` 之外，联合类型的值不能直接做 `+` 或调方法（因为编译器不知道当前是哪种）。先 `match` 收窄到具体类型再用。

约束：

- 成员须**两两互不相交**：`i64 | i64`、`&i64 | &mut i64`、`i64 | isize` 报 `UnionMembersNotDisjoint`（编译器无法区分）。
- 优先级：`&T | &mut U` = `(&T) | (&mut U)`；注意闭包注解 `|x: i64| ..` 的 `|` 是闭包符号，**不是**联合运算符。

---

## 5.5 枚举显式判别式（U3 ✅）

枚举变体可带显式判别式——判别值即该变体的 tag，构造和 `match` 都复用：

```rlyeh
enum Code { Ok = 200, NotFound = 404, Error = 500 }

let c = Code::Ok;
match c {
    Code::Ok => println(200),
    Code::NotFound => println(404),
    Code::Error => println(500),
}
```

> **C 程序员的视角**：这正好对应 C 的 `enum Code { Ok = 200, NotFound = 404, Error = 500 };`——给每个枚举常量指定整数值（常用于 HTTP 状态码、协议字段等需要和外部数值对齐的场景）。

---

## 5.6 综合示例：用 enum + match 建模状态机

```rlyeh
enum State {
    Idle,
    Running { speed: f64 },
    Error(String),
}

fn describe(s: State) -> String {
    match s {
        State::Idle => String::from("空闲"),
        State::Running { speed } => {
            let mut msg = String::from("运行中，速度=");
            msg = msg + speed;
            msg
        },
        State::Error(e) => {
            let mut msg = String::from("错误：");
            msg = msg + e;
            msg
        },
    }
}
```

> 这个模式在真实项目里极常见：用 `enum` 表达"状态的有限集合"，用 `match` 穷尽处理每一种，编译器保证你不会漏掉任何一种状态（C 的 `switch` 做不到这点）。

---

## 练习

1. 运行 [`examples/by-chapter/05-aggregates-generics.rl`](../../examples/by-chapter/05-aggregates-generics.rl)，给 `Shape` 加一个 `Triangle(f64, f64)` 变体并在 `area` 里处理它，体会 `match` 的穷尽检查（漏写会编译报错）。
2. 写一个 `trait Draw { fn draw(&self); }`，让 `Circle`/`Rect` 都实现，再用 `dyn Draw` 把多个形状放进一个集合统一绘制。
3. 用类型联合 `i64 | String` 模拟"可能是数字也可能是文本"的字段，并 `match` 分别处理两种类型。

---

[← 上一章：数学式条件判断](./04-math-conditions.md) | [返回指南目录](./index.md) | [下一章：数组与索引 →](./06-arrays-slices.md)
