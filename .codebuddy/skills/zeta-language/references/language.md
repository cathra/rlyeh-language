# Zeta 语言语法速查（v0.1.0 MVP）

> 权威规范：`docs/grammar.md`（EBNF）、`docs/guide.md`（教程）。本节是编写代码时的速查。

## 1. 词法

- 注释：`// 行注释`、`/* 块注释 */`
- 整数默认 `i64`，浮点默认 `f64`；布尔 `true/false`；字符 `'a'`；字符串 `"..."`（无转义插值）
- 时间字面量：`9am` = 540、`6pm` = 1080（分钟整数，可参与算术）
- 标识符：字母/下划线开头，字母数字下划线

## 2. 类型

标量：`i8 i16 i32 i64 isize u8 u16 u32 u64 usize f32 f64 bool char` + 无类型 `()`。
聚合：`[T; N]` 定长数组、`Vec<T>`（标准库）、`String`（标准库）、`Str`（**未实现**，勿用）、`&T`（**未实现**，仅方法接收者 `&self`/`&mut self`）。

类型注解：
```zeta
let x: i64 = 10;
let arr: [i64; 4] = [1, 2, 3, 4];
```

## 3. 变量与常量

```zeta
let mut count = 0;        // mut 可变
const PI: f64 = 3.14159;  // 模块/函数级常量
```
`let` 绑定按值拷贝（见 semantics.md 缓冲共享语义）。

## 4. 函数

```zeta
fn add(a: i64, b: i64) -> i64 { a + b }
fn main() { println(add(1, 2)); }   // 程序必须显式 main
extern fn my_lib_fn(x: i64) -> i64; // FFI 声明（无 body，链接器解析）
```
- 支持 `return`、`break`、`continue`；表达式可作返回值（`{ a + b }` 尾表达式）。

## 5. 控制流

```zeta
if 0 < x < 10 { println("range"); } else { println("out"); }   // 无需括号，语句用 ; 分隔
while x > 0 { x -= 1; }
loop { if done { break; } }
for i in 0..<10 { println(i); }     // 仅数值区间（半开/双闭）
match s {
    Shape::Circle(r) => 3.14 * r * r,
    Shape::Rect { w, h } => w * h,
}
```

## 6. 比较链与 in 表达式（语言特色）

```zeta
if 0 < x < 10 {}        // 正向区间链 = 0 < x && x < 10
if 0 <= x <= 10 {}      // 双闭
if 0 > x > 10 {}        // 反向链 = x < 0 || x > 10（区间外）
if x in (1, 3, 5) {}            // 离散集合成员（展开为 == 链）
if ch in ('a'..<'z', 'A'..<'Z') {}  // 范围元素展开
if x in 0..<10 {}       // 裸范围 = 区间判断 [0, 10)
if x in 0...10 {}       // [0, 10]
if x in 0<..10 {}       // (0, 10]
if x not in (6am..<10pm) {}     // 跨午夜时间区间（分钟单位）
```

## 7. 运算符与优先级

算术 `+ - * / %`；比较 `== != < <= > >=`；逻辑 `&& || not`；位运算 `& | ^ << >> ~`。

> **优先级**（从高到低）：`*` > `+` > `<<` > `&` > `^` > `|`，且**比较运算高于位运算**。
> 裸 `x & 3 == 2` 会把 `&` 当逻辑与处理——必须写 `(x & 3) == 2`。

## 8. 聚合类型（struct / enum / trait / 泛型）

```zeta
struct Point { x: i64, y: i64 }
enum Shape { Circle(f64), Rect { w: f64, h: f64 } }

trait Area { fn area(&self) -> f64; }
impl Area for Shape {
    fn area(&self) -> f64 {
        match self {
            Shape::Circle(r) => 3.14 * r * r,
            Shape::Rect { w, h } => w * h,
        }
    }
}
let p = Point { x: 1, y: 2 };   // 结构体字面量
```
- 泛型通过单态化实现（`Vec<T>`、`Option<T>`、`Result<T, E>`）。
- `match` 模式：枚举变体 `Enum::Variant(...)`、结构字段 `{ w, h }`、嵌套 `Option<Vec<T>>` 均支持（具体实例化）。

## 9. 数组、切片与索引

```zeta
let arr = [10, 20, 30];
arr[0] = 99;                 // 索引读写（别名共享，互相可见）
let ch = s[0];               // String 按字符索引（步长 1 字节，ASCII 假设）
let a = arr[1..<3];          // 半开 [1,3) → [20, 30]（返回全新缓冲，按值拷贝）
let b = v[lo...hi];          // Vec 动态切片（越界 clamp 到 [0, len]，start>=end 空）
```

## 10. 模块系统

```zeta
mod math {
    pub const PI: f64 = 3.14159;
    pub fn square(x: i64) -> i64 { x * x }
}
use math::PI;
use math::square as sq;
// 多文件：mod foo; → foo.zeta / foo/mod.zeta；跨模块 mod::Enum::Variant
```

## 11. 区域（Region）

```zeta
region 'r {
    let data = BigStruct::new() in 'r;
    process(&data);          // 块结束批量释放
}
region 'r adaptive {         // 智能分配（编译器推断大小）
    for i in 0..<10000 { let obj = Data::new(i) in 'r; }
}
fn make() -> BigStruct {
    region 'r {
        let d = BigStruct::new() in 'r;
        return transfer d out of 'r;   // 所有权转移出区域
    }
}
```

## 12. Actor 并发

```zeta
actor Counter {
    value: i64 = 0,
    pub fn increment(amount: i64) -> i64 {
        self.value += amount;
        self.value
    }
}
let c = Counter::new();                    // 普通 spawn
let r = c.increment(10).await;             // ask 同步往返
send c.increment(1);                       // fire-and-forget
let s = Counter::new_supervised(0);        // 受监督：0=OneForOne 1=AllForOne 2=RestartForOne
```
- 方法返回 `-1` = 崩溃信号（ask 返回 0，supervisor 重建重启，无监督则停止）。
- 消息经「kind 槽 + 3 个 i64 槽」传递，同 actor 消息按邮箱 FIFO 互斥。

## 13. FFI（extern fn）

```zeta
extern fn abs(x: i64) -> i64;      // 声明，无 body
extern fn srand(seed: u32);        // void 返回类型
```
- codegen 生成 `declare`（链接器解析）；extern 符号必须与 libc 一致。
- 支持标量类型 + `()`；`&T`/指针类型用 `Ptr`（未知名类型回退）。

## 14. 内建函数（无需导入）

- `println(expr)`：打印（**单参数，无 `{}` 格式化**）
- 长度：`.len` 字段或 `.len()` 方法（String / Vec / HashMap；**无内建 `len` 函数**）

## 15. 最小完整示例

```zeta
fn main() {
    let mut total = 0;
    for i in 0..<5 { total += i; }
    if total in (0...20) { println(total); }
}
```
