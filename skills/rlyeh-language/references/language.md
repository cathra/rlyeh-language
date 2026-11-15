# Rlyeh 语言语法速查（v0.2.0）

> 权威规范：`docs/grammar.md`（EBNF）、`docs/guide/index.md`（教程）。本节是编写代码时的速查。

## 1. 词法

- 注释：`// 行注释`、`/* 块注释 */`
- 整数默认 `i64`，浮点默认 `f64`；布尔 `true/false`；字符 `'a'`；字符串 `"..."`（无转义插值）
- 时间字面量：`9am` = 540、`6pm` = 1080（分钟整数，可参与算术）
- 标识符：字母/下划线开头，字母数字下划线

## 2. 类型

标量：`i8 i16 i32 i64 isize u8 u16 u32 u64 usize f32 f64 bool char` + 无类型 `()`。
聚合：`[T; N]` 定长数组、`Vec<T>`、`String`、`str`/`&str`（字符串字面量与只读视图）、`&T` / `&mut T`、`&[T]` / `&mut [T]`（零拷贝胖指针视图 `{ data, len }`）、元组 `(A, B, ...)`。

类型注解：
```rlyeh
let x: i64 = 10;
let arr: [i64; 4] = [1, 2, 3, 4];
```

## 3. 变量与常量

```rlyeh
let mut count = 0;        // mut 可变
const PI: f64 = 3.14159;  // 模块/函数级常量
```
`let` 绑定按值拷贝（见 semantics.md 缓冲共享语义）。

## 4. 函数

```rlyeh
fn add(a: i64, b: i64) -> i64 { a + b }
fn main() { println(add(1, 2)); }   // 程序必须显式 main
extern fn my_lib_fn(x: i64) -> i64; // FFI 声明（无 body，链接器解析）
```
- 支持 `return`、`break`、`continue`；表达式可作返回值（`{ a + b }` 尾表达式）。

## 5. 控制流

```rlyeh
if 0 < x < 10 { println("range"); } else { println("out"); }   // 无需括号，语句用 ; 分隔
while x > 0 { x -= 1; }
loop { if done { break; } }
for i in 0..<10 { println(i); }     // 数值区间（半开/双闭）
for x in vec { println(x); }        // 数组 / Vec / 自定义迭代器
for (k, v) in map { println(k); }   // HashMap 键值对
match s {
    Shape::Circle(r) => 3.14 * r * r,
    Shape::Rect { w, h } => w * h,
}
```

## 6. 比较链与 in 表达式（语言特色）

```rlyeh
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

算术 `+ - * / %`；比较 `== != < <= > >=`；逻辑 `&& || not`；位运算 `& | ^ << >> ~`；类型转换 `as`（如 `i64 as f64`，U6 ✅）。

> **优先级**（从高到低）：`*` > `+` > `<<` > `&` > `^` > `|`，且**比较运算高于位运算**。
> 裸 `x & 3 == 2` 会把 `&` 当逻辑与处理——必须写 `(x & 3) == 2`。

## 8. 聚合类型（struct / enum / protocol / 泛型）

```rlyeh
struct Point { x: i64, y: i64 }
enum Shape { Circle(f64), Rect { w: f64, h: f64 } }

protocol Area { fn area(&self) -> f64; }
impl Shape: Area {
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
- 类型联合 `A | B`（U1/U2 ✅）：成员两两互不相交（`i64 | isize` 报 `UnionMembersNotDisjoint`），`match` 用类型臂解构（`i64 => ...`）。
- 枚举显式判别式（U3 ✅）：`enum Code { Ok = 200, NotFound = 404 }`，判别值即变体 tag。

## 9. 数组、切片与索引

```rlyeh
let arr = [10, 20, 30];
arr[0] = 99;                 // 索引读写（别名共享，互相可见）
let ch = s[0];               // String 按字符索引（步长 1 字节，ASCII 假设）
let a = arr[1..<3];          // 半开 [1,3) → [20, 30]（返回全新缓冲，按值拷贝）
let b = v[lo...hi];          // Vec 动态切片（越界 clamp 到 [0, len]，start>=end 空）
```

- 切片引用 `&[T]` / `&mut [T]`（S1/S2/S3 ✅）：零拷贝胖指针视图（`{ data, len }`），`&[T; N]` 经 unsize coercion → `&[T]`；方法 `.len()`/`.first()`/`.last()`/`.iter()`；`Vec::as_slice()`/`as_mut_slice()`。动态切片 `v[lo..<hi]` 返回全新缓冲（值拷贝），与切片引用语义不同。

## 10. 模块系统

```rlyeh
module math {
    pub const PI: f64 = 3.14159;
    pub fn square(x: i64) -> i64 { x * x }
}
import math::PI;
import math::square as sq;
import math::{square, PI};      // 组导入
import math::*;                 // glob（注意同名歧义）
// 多文件：module foo; → foo.rl / foo/module.rl；嵌套 module 与 pub use 重导出已支持
// 扁平名字空间：无 crate/super/self，路径即「模块名::」；B4 默认私有 + pub 校验灰度可用
```

## 11. 区域（Region）

```rlyeh
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

```rlyeh
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

```rlyeh
extern fn abs(x: i64) -> i64;      // 声明，无 body
extern fn srand(seed: u32);        // void 返回类型
```
- codegen 生成 `declare`（链接器解析）；extern 符号必须与 libc 一致。
- `extern` 调用必须在 `unsafe { }` 内（std prelude 内部 FFI 豁免）。
- 支持标量类型 + `()`；`&T`/指针类型用 `Ptr`（未知名类型回退）。

## 14. 内建函数（无需导入）

- `println(expr)`：打印（**单参数，无 `{}` 格式化**）
- 长度：`.len` 字段或 `.len()` 方法（String / Vec / HashMap；**无内建 `len` 函数**）

## 15. 最小完整示例

```rlyeh
fn main() {
    let mut total = 0;
    for i in 0..<5 { total += i; }
    if total in (0...20) { println(total); }
}
```

## 16. 0.2.0 新增能力（速查）

```rlyeh
// 元组值/解构/多返回
fn pair() -> (i64, String) { (1, String::from("a")) }
let (a, b) = pair();

// if let / while let / match 守卫 + 或/范围模式
if let Option::Some(x) = o { println(x); }
while let Option::Some(x) = it.next() { println(x); }
match n {
    1 | 2 => println("small"),
    3...9 if n % 2 == 0 => println("even"),
    _ => println("other"),
}

// derive
#[derive(Clone, PartialEq, Debug)]
struct P { x: i64, y: i64 }

// const / static / unsafe / repr(C)
const LIMIT: i64 = 10;
static mut FLAG: i64 = 0;
#[repr(C)] struct Header { tag: u8, len: i64 }
let p: *const i64 = &x;
let v = unsafe { *p };

// Drop/RAII（Q1–Q3）
impl Guard: Drop { fn drop(&mut self) { self.unlock(); } }

// mem / ? / try
mem::swap(&mut a, &mut b);
mem::replace(&mut a, 0);
mem::take(&mut a);
let v = may_fail()?;
let r = try { may_fail()?; 42 };   // Result 错误聚合块
```

- 泛型 protocol/impl、嵌套模块、`pub use` 已支持；`Box<dyn Any + Send>`/多 protocol 约束未实现。
- `-O0/-O1/-O2/-O3/-Os` 与 `--no-heap` 见 SKILL.md 与 `docs/optimization-levels.md` / `docs/no-heap.md`。
