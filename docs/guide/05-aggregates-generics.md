# 5. 聚合类型与泛型

## 5.1 结构体

```rlyeh
struct Point {
    x: f64,
    y: f64,
}

let p = Point { x: 1, y: 2 };
println(p.x);                      // 1
```

## 5.2 枚举与模式匹配

```rlyeh
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

模式支持：枚举变体 `Enum::Variant(...)`、结构字段 `{ w, h }`、嵌套 `Option<Vec<T>>` 均支持（具体实例化）。具名变体可带字段：

```rlyeh
enum Message {
    Quit,
    Move { x: i64, y: i64 },
    Write(String),
}

// 匹配：
match msg {
    Message::Quit => println(0),
    Message::Move { x, y } => println(x + y),
    Message::Write(s) => println(s.len()),
}
```

## 5.3 trait 与泛型（单态化）

```rlyeh
trait Area {
    fn area(&self) -> f64;
}

impl Area for Shape {
    fn area(&self) -> f64 {
        // ...
    }
}

struct Wrapper<T> {
    value: T,
}

impl<T> Wrapper<T> {
    fn new(v: T) -> Wrapper<T> {
        Wrapper { value: v }
    }
}

// 泛型 trait 约束（bound / where，✅）
trait Convert<T> {
    fn convert(&self) -> T;
}

impl Convert<i64> for f64 {
    fn convert(&self) -> i64 {
        *self as i64
    }
}

// 泛型 impl 单态化后 Self 替换为具体类型（Wrapper<i64>::new(v) -> Self → Wrapper<i64>）。
// MVP 限制：Self 仅支持返回位置——参数位置（关联返回）保持禁止；dyn 场景含 Self 签名方法不可经 vtable 调用（H4 既有限制）；
// From::from(v) -> Self / Into::into() -> Self / Deserialize::from_json(s) -> Self 落地待 std trait 声明。
```

### 5.4 类型联合 `A | B`（U1 / U2 ✅，2026-08-30）

类型联合 `A | B` 表示「值为 A 或 B」，运行时为匿名 enum（槽 0 = tag、槽 1 = payload），复用现有 enum codegen：

```rlyeh
let x: i64 | String = 5;            // 成员值直接构造联合（协变）
match x {
    i64 => println(i64),            // 类型臂：payload 绑定到类型名同名变量
    String => println(String.len()),
}
```

- 成员须**两两互不相交**：`i64 | i64`、`&i64 | &mut i64`、`i64 | isize` 报 `UnionMembersNotDisjoint`。
- 优先级：`&T | &mut U` = `(&T) | (&mut U)`；闭包注解 `|x: i64| ..` 的 `|` 非联合运算符。
- 未收窄的联合禁止直接运算 / 方法调用（`compatible_with` 单向：成员 → 联合）。

### 5.5 枚举显式判别式（U3 ✅，2026-08-30）

枚举变体可带显式判别式，判别值即该变体的 tag，构造与 `match` 均复用：

```rlyeh
enum Code { Ok = 200, NotFound = 404, Error = 500 }
let c = Code::Ok;
match c {
    Code::Ok => println(200),
    Code::NotFound => println(404),
    Code::Error => println(500),
}
```

---

[← 上一章：数学式条件判断](./04-math-conditions.md) | [返回指南目录](./index.md) | [下一章：数组与索引 →](./06-arrays-slices.md)
