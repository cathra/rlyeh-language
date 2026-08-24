# Zeta 语言教程指南

> 版本：v2.0（MVP）
> 面向从零开始的读者：本教程带你安装工具链、写出第一个程序，并系统掌握 Zeta 的核心特性。
> 所有示例均取自 `examples/` 与 `tests/run-pass/` 中的可编译运行代码。
> 完整参考见 [manual.md](./manual.md)；权威规范见 `grammar.md` / `semantics.md` / `memory-model.md` / `actor-model.md` / `std-lib.md`。

---

## 目录

1. [认识 Zeta](#1-认识-zeta)
2. [工具链部署与安装](#2-工具链部署与安装)
3. [第一个程序](#3-第一个程序)
4. [基础语法](#4-基础语法)
5. [数学式条件判断](#5-数学式条件判断)
6. [聚合类型与泛型](#6-聚合类型与泛型)
7. [数组、字符串与集合](#7-数组字符串与集合)
8. [模块系统](#8-模块系统)
9. [内存管理：分层所有权模型](#9-内存管理分层所有权模型)
10. [Actor 并发模型](#10-actor-并发模型)
11. [错误处理与序列化](#11-错误处理与序列化)
12. [输入输出、网络与时间](#12-输入输出网络与时间)
13. [实战：创建并发布一个项目](#13-实战创建并发布一个项目)

---

## 1. 认识 Zeta

Zeta 是一门面向未来十年基础设施的**系统级编程语言**：

| 设计目标 | 对标 | 落地形态 |
|----------|------|----------|
| 内存安全、零 GC | Rust | 分层内存模型（L0 所有权 → L1 区域 → L2 Rc → L3 GC） |
| 编译速度极快 | Go | 模块级缓存（增量编译） |
| 并发模型一等公民 | Erlang / Akka | `actor` 语言级构造 + 运行时监督 |
| 数学式语法直觉 | Python / MATLAB | 比较链 `0 < x < 10`、集合判断 `x in (1, 3, 5)` |

**一句话定位**：Zeta = Rust 的安全性 + Go 的编译速度 + Erlang 的并发模型 + 数学的自然表达。

当前编译器为 Rust 实现的 bootstrap 阶段，代码生成走 LLVM IR。

---

## 2. 工具链部署与安装

### 2.1 前置依赖

| 依赖 | 用途 |
|------|------|
| Rust 工具链（`cargo`/`rustc` ≥ 1.75） | 编译器自举构建 |
| LLVM/Clang（macOS 自带或 `xcode-select --install`） | 汇编与链接后端 |
| git | 版本信息 |

```bash
cargo --version && rustc --version && clang --version
```

### 2.2 一键构建工具链

仓库根目录的 `toolchains/` 目录承载完整的构建、发布、归档流程：

```bash
cd zeta-language
./toolchains/build.sh            # 完整流程：环境检查 → release 构建 → 测试 → 发布 → 冒烟 → 归档
```

常用变体：

```bash
./toolchains/build.sh --no-test              # 跳过测试（快速迭代）
./toolchains/build.sh --no-install           # 仅构建 + 归档，不发布
./toolchains/build.sh --prefix /opt/zeta     # 自定义安装前缀
```

构建产物归档在 `toolchains/dist/zeta-toolchain-<ver>-<os>-<arch>.tar.gz`，可分发到任意机器（解压后即可用，`bin` 与 `std` 同级、可重定位）。

### 2.3 本地发布（install.sh）

```bash
./toolchains/install.sh                # 发布到默认 ~/.zeta
ZETA_PREFIX=/opt/zeta ./toolchains/install.sh   # 自定义前缀
```

安装布局：

```
<prefix>/
├── bin/
│   ├── zeta              # 编译器入口命令（wrapper 自动定位标准库）
│   ├── zeta-driver       # 编译器本体
│   ├── zeta-fmt / zeta-check / zeta-doc / zeta-bench
│   └── zep               # 包管理器
├── std/                  # 标准库源码（core.zeta + time/io/net/sync/fs 模块）
└── registry/             # 本地 zep 注册表（publish 目标）
```

### 2.4 配置 PATH

```bash
export PATH="$HOME/.zeta/bin:$PATH"    # 建议写入 ~/.zshrc / ~/.bashrc
zeta --version                          # 验证：zeta 0.1.0
```

### 2.5 交叉编译环境（可选）

```bash
# WASM 目标（actor 程序亦支持）
rustup target add wasm32-wasip1
cargo build --release --target wasm32-wasip1 -p zeta-actor-runtime
brew install wasi-libc lld             # WASI sysroot 与 wasm-ld
```

---

## 3. 第一个程序

```zeta
// hello-world.zeta
fn main() {
    println("Hello, Zeta!");
}
```

编译与运行：

```bash
zeta build hello-world.zeta     # 生成可执行文件 hello-world
./hello-world
zeta run hello-world.zeta       # 编译并运行（一步到位）
```

---

## 4. 基础语法

### 4.1 变量与类型

```zeta
let x = 6;                       // 类型推断：i64
let y: f64 = 3.14;               // 显式类型注解
let mut v = Vec::new();          // 可变绑定（对象修改用）
```

标量类型：`i8/i16/i32/i64/isize`、`u8/u16/u32/u64/usize`、`f32/f64`、`bool`、`char`、`()`。

### 4.2 函数

```zeta
fn add(a: i64, b: i64) -> i64 {
    a + b                        // 末表达式为返回值（隐式 return）
}

fn main() {
    let sum = add(6, 7);         // 13
}
```

**函数一等值（函数指针）**：函数可以像值一样绑定、传递与调用：

```zeta
fn apply(f: fn(i64, i64) -> i64, x: i64, y: i64) -> i64 {
    f(x, y)
}

fn main() {
    let f = add;                 // 函数值绑定
    let r1 = f(3, 4);            // 7
    let r2 = apply(add, 10, 20); // 30
}
```

**无捕获闭包**：`|x, y| 表达式` desugar 为匿名函数 + 函数指针（零运行时开销）：

```zeta
let r1 = apply(|a, b| a + b, 10, 20);   // 30
let inc: fn(i64) -> i64 = |x| x + 1;    // 注解绑定闭包
let r2 = inc(41);                       // 42
```

**捕获闭包（IIFE）**：立即调用形式，外层变量按值捕获：

```zeta
let factor = 3;
let r1 = (|x| x * factor)(14);          // 42
```

**闭包值对象**：`let f = |x: i64| ...;` 绑定后可反复调用：

```zeta
let inc = |x: i64| x + 1;
let r1 = inc(41);                       // 42
let base = 40;
let add_base = |x: i64| x + base;       // 按值捕获
let r2 = add_base(2);                   // 42
```

**trait 对象（`dyn Trait`）**：2 槽胖指针（数据指针 + vtable），`&T` 可强制转换为 `dyn Trait`：

```zeta
trait Shape {
    fn area(&self) -> f64;
}

struct Circle { radius: f64 }
impl Shape for Circle {
    fn area(&self) -> f64 { 3.14 * self.radius * self.radius }
}

let c = Circle { radius: 2.0 };
let d: dyn Shape = &c;
println(d.area());              // 12.56（vtable 分派）
```

### 4.3 控制流

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

for i in 0..<10 {        // 数值区间：半开 [0, 10)
    println(i);          // 0 1 2 ... 9
}

for j in 1...3 {         // 双闭区间 [1, 3]
    println(j);          // 1 2 3
}
```

`for` 还支持 `for x in vec`、`for (k, v) in map`、`for x in arr`。`break` / `continue` 均可用。`match` 用于枚举解构。

### 4.4 注释

```zeta
// 行注释
/// 文档注释（zeta doc 提取生成 Markdown）
```

---

## 5. 数学式条件判断

### 5.1 区间内（正向比较链）

```zeta
if 0 < x < 10 {}      // 0 < x && x < 10
if 0 <= x <= 10 {}    // 0 <= x && x <= 10
```

### 5.2 区间外（反向比较链 = 并集）

```zeta
if 0 > x > 10 {}      // x < 0 || x > 10
```

### 5.3 集合判断

```zeta
if x in (1, 3, 5) {}                          // x == 1 || x == 3 || x == 5
if ch in ('a'..<'z', 'A'..<'Z') {}            // 范围展开为离散成员
if x not in (0..<10) {}                       // 非全部（!x in）
```

### 5.4 裸范围 = 区间判断

```zeta
if x in 0..<10 {}    // 0 <= x < 10     [0, 10)
if x in 0...10 {}    // 0 <= x <= 10    [0, 10]
if x in 0<..10 {}    // 0 < x <= 10     (0, 10]
```

### 5.5 时间字面量

```zeta
if hour in (9am...6pm) {}             // 工作时间段
if hour not in (6am..<10pm) {}        // 跨午夜自动处理
```

---

## 6. 聚合类型与泛型

### 6.1 结构体

```zeta
struct Point {
    x: i64,
    y: i64,
}

let p = Point { x: 1, y: 2 };
```

### 6.2 枚举与匹配

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

`match` 支持嵌套泛型载荷解构（如 `Option<Vec<T>>`、`Result<Option<String>, i64>`）。

### 6.3 trait 与泛型

```zeta
trait Area {
    fn area(&self) -> f64;
}

impl Area for Shape {
    fn area(&self) -> f64 { /* ... */ }
}
```

泛型按**单态化**编译（每个具体类型实例化一份代码）。

---

## 7. 数组、字符串与集合

### 7.1 数组与索引

```zeta
let arr = [10, 20, 30];                 // 数组字面量
let arr2: [i64; 4] = [1, 2, 3, 4];      // 类型注解保留长度
let x = arr[0];                         // 索引读取（越界编译期可查）
arr[1] = 99;                            // 索引写入
```

**动态切片**：`v[lo..<hi]`（半开）/ `v[lo...hi]`（双闭）返回全新缓冲，越界自动 clamp：

```zeta
let arr = [10, 20, 30, 40, 50];
let a = arr[1..<3];              // [20, 30]
```

### 7.2 String

```zeta
let s = String::from("Hello, Zeta");
s.len                       // 11（字节长度）
s[0]                        // 按字节索引
s.substring(0, 5)           // "Hello"
s.contains("Zeta")          // true（字面量实参自动升级）
s.starts_with("Hello")      // true
s.replace("Hello", "Hi")    // "Hi, Zeta"
s.to_upper() / s.to_lower() / s.trim()
s.split(",")                // Vec<String>
s.as_str()                  // &str 只读借用视图（零拷贝）
int_to_string(42)           // "42"
string_to_int("42")         // 42
```

> **实参自动升级**：`String` 形参位置传入字符串字面量自动构造 String——`m.push_str("!")` 直接可用。
> **拼接与比较**：`s + "!"`、`s == "hi"` 自动升级为 String 语义。

### 7.3 Vec

```zeta
let mut v: Vec<i64> = Vec::new();   // 建议显式类型注解
v.push(10); v.push(20); v.push(30);
v.len / v.cap                // 字段；v.is_empty() 为方法
v[0] / v.set(i, x)           // 索引读写
v.pop()                      // Option<T>
v.contains(20) / v.find(20)  // bool / 下标（未找到 -1）
v.remove(i) / v.insert(i, x)
v.sort() / v.reverse() / v.swap(i, j)
v.first() / v.last() / v.binary_search(20)
for x in v { }               // 容器迭代
```

### 7.4 HashMap

```zeta
let mut m: HashMap<String, i64> = HashMap::new();
m.insert("k", 42);                  // 键字面量自动升级
m.contains_key("k") / m.get("k") / m.remove("k")
m.len / m.cap
for (k, v) in m { }                 // 容器迭代
```

### 7.5 Option / Result

```zeta
let x = Some(10);
x.is_some() / x.is_none() / x.unwrap() / x.unwrap_or(0) / x.expect("msg")

let r: Result<i64, i64> = Ok(7);
r.is_ok() / r.is_err() / r.unwrap() / r.unwrap_or(0)
```

---

## 8. 模块系统

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

## 9. 内存管理：分层所有权模型

| 层级 | 机制 | 关键字 | 运行时开销 |
|------|------|--------|------------|
| L0 | 静态所有权 + 借用 + 生命周期 | `let`, `&`, `&mut` | 零开销 |
| L1 | 区域（Region） | `region 'r {}`, `in 'r`, `transfer` | 零开销（批量释放） |
| L2 | 引用计数 | `Rc<T>`, `Arc<T>` | 原子操作 |
| L3 | 可选 GC | `Gc<T>`（插件式） | GC 停顿 |

### 9.1 L0：所有权与借用

```zeta
let s = String::from("hello");
let t = s;               // 值拷贝：共享底层缓冲，s 仍可用
let r = &t;              // 不可变引用（多个 & 可共存）
fn first_char_len(s: &String) -> i64 { s.len() }
let n = *r;              // 显式解引用（标量）
```

严格借用检查（NLL 近似）：`&mut T` 与任何活跃借用互斥；`&mut` 要求 `let mut` 绑定；局部引用逃逸报错；读取被借用变量允许。

`ref` / `ref mut` 模式：绑定为对匹配值的引用而非拷贝。

```zeta
let x = 42;
match x { ref r => println(*r) }        // 42：r: &i64
```

### 9.2 L1：区域（Region）

区域内的对象随区域退出**批量释放**（零开销）：

```zeta
region 'r {
    let data = BigStruct::new() in 'r;
    process(&data);
} // 批量释放

fn create_data() -> BigStruct {
    region 'r {
        let data = BigStruct::new() in 'r;
        return transfer data out of 'r;    // 所有权移出区域
    }
}
```

智能分配：`region 'r adaptive { ... }`（PGO 画像建议容量）、`region 's with_size (4096)`、`region 't strategy (bump)`。

### 9.3 堆分配：`Box<T>` / `Rc<T>` / `Arc<T>`

```zeta
let b = Box::new(42);                 // 标量装箱
println(*b);                          // 42
let bp = Box::new(Point { x: 10, y: 20 });
println(bp.x + bp.y);                 // 30（字段访问自动剥 Box）

let r = Rc::new(42);
let r2 = r.clone();
println(r.strong_count());            // 2
let w = r.downgrade();
match w.upgrade() { Option::Some(rc) => println(*rc), Option::None => println(0) }
```

### 9.4 L3：`Gc<T>`（可选 GC）

```zeta
gc_region {
    let a = Gc::new(42);
    println(*a);                      // 42
} // 块结束触发 GC 周期

let g = gc_region { let inner = Gc::new(100); inner };  // 逃逸对象存活到块外
println(*g);                          // 100
```

---

## 10. Actor 并发模型

`actor` 是语言级构造：actor 方法消息经「kind 槽 + 3 个 i64 消息槽」传递，同一 actor 的邮箱按 FIFO 互斥处理。

### 10.1 定义与调用

```zeta
actor Counter {
    value: i64 = 0,

    pub fn increment(amount: i64) -> i64 {
        self.value += amount;
        self.value
    }
}

fn main() {
    let c = Counter::new();            // 普通 spawn
    let r = c.increment(10).await;     // ask 同步往返
    println(r);                        // 10
    send c.increment(1);               // fire-and-forget 异步发送
}
```

- `new()`：生成 `__state_new` + `zeta_actor_spawn`
- 方法调用 `.await`：ask 同步往返；`send expr`：异步入队
- 消息按 FIFO 处理，`send` 后立刻 `ask` 能看到累积状态

### 10.2 监督与崩溃恢复

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
    println(m.explode().await);          // 0（ask 立即失败）
    println(m.tick(3).await);            // 3（已重启，状态回到初始值）
}
```

崩溃协议：方法返回 `-1` → 判定 Panic → ask 立即返回 0；受监督 actor 由 supervisor 重建初始状态并重启。

### 10.3 异步函数 `async fn` / `await`

```zeta
async fn get_value(x: i64) -> i64 {
    x * 2
}

async fn nested(x: i64) -> i64 {
    let v = get_value(x).await;
    v + 1
}

fn main() {
    let mut fut1 = get_value(21);
    println(block_on(&mut fut1));   // 42
    let mut fut2 = nested(10);
    println(block_on(&mut fut2));   // 21
}
```

`async fn` desugar 为 Future 结构体 + poll 状态机；`.await` 挂起/恢复；跨 await 变量提升为结构体字段。MVP 限制：参数/返回限 `i64`/`()`。

### 10.4 同步原语

```zeta
let mutex = Mutex::new();
mutex.lock();
// 临界区
mutex.unlock();
mutex.try_lock()            // 已锁返回 0

let rw = RwLock::new();
rw.read_lock();  rw.unlock();
rw.write_lock(); rw.unlock();
```

---

## 11. 错误处理与序列化

### 11.1 `?` 错误传播

```zeta
fn try_div(x: i64, y: i64) -> Option<i64> {
    if y == 0 { return None; }
    Some(x / y)
}

fn chain(a: i64, b: i64, c: i64) -> Option<i64> {
    let q = try_div(a, b)?;        // 解包；失败则 return None
    Some(q + 1)
}
```

### 11.2 JSON 序列化

```zeta
struct Point { x: i64, y: i64 }

println(json::stringify(42));                          // 42
println(json::stringify(true));                        // true
println(json::stringify("hi"));                        // "hi"
println(json::stringify([1, 2, 3]));                   // [1,2,3]

let p = Point { x: 10, y: 20 };
println(json::stringify(p));                           // {"x":10,"y":20}

println(json::parse::<i64>("42"));                     // 42
println(json::parse::<bool>("true"));                  // true
let m2 = json::parse::<HashMap<i64, i64>>("{\"1\":10,\"2\":20}");
```

---

## 12. 输入输出、网络与时间

### 12.1 文件 IO

```zeta
let r = read_file("data.txt");                       // Result<String, IoError>
match r {
    Ok(content) => { let _ = write_file("out.txt", content); }
    Err(e) => println(e.message()),
}
let _ = append_file("log.txt", line);                // 追加
let line = read_line();                              // stdin 读一行
```

### 12.2 网络

```zeta
let host = hostname();                     // 本机主机名
let pair = socketpair_stream();            // AF_UNIX 全双工 fd 对
let _ = send_all(fd, data);                // 发送
let got = recv_some(fd, n);                // 接收
let fd = tcp_connect(8080, 127, 0, 0, 1);  // TCP 连接
```

### 12.3 时间

```zeta
let d = Duration { micros: 1500000 };
d.secs()        // 1
d.millis()      // 1500
d.micros()      // 1500000
d.nanos()       // 1500000000

let t = Instant::now();
// ... 计算 ...
let elapsed = t.elapsed();   // Duration
```

### 12.4 位运算与内建打印

```zeta
let port = (0xC0A8 << 8) | 0x010A;   // 字节打包
let byte = (port >> 8) & 0xFF;

println("Hello, Zeta!");   // 字符串
println(42);               // 整型
println(3.14);             // 浮点
println(true);             // bool
println();                 // 空行
print(x);                  // 不换行
```

> 位运算优先级：`*` > `+` > `<<` > `&` > `^` > `|`，比较 `>` 位运算，`(x & 3) == 2` 需要括号。

---

## 13. 实战：创建并发布一个项目

### 13.1 创建项目脚手架

```bash
zeta new myapp          # 生成 Zeta.toml + src/main.zeta
cd myapp
zeta run src/main.zeta  # 编译并运行
```

### 13.2 格式化、检查与文档

```bash
zeta fmt src/main.zeta -w          # 格式化并写回
zeta fmt src/main.zeta --check     # 仅检查（CI 用）
zeta check src/main.zeta           # 静态分析
zeta doc lib.zeta --out docs/api.md   # 从 /// 注释生成文档
```

### 13.3 基准测试

```bash
zeta bench fib.zeta --runs 10 --warmup 2   # 编译并基准计时
# 或独立工具
zeta-bench fib.zeta --runs 5
```

### 13.4 依赖管理（zep）

```bash
zep add foo@^1.0       # 添加依赖（解析到 Zeta.lock）
zep update             # 重新解析
zep build && zep run
```

### 13.5 发布到本地注册表

```bash
zeta publish           # 默认发布到 ~/.zeta/registry
zep search myapp       # 注册表中搜索
```

### 13.6 测试用例目录

`zeta test [<tests-dir>]` 运行目录下 `compile-pass/`、`compile-fail/`、`run-pass/` 三类用例（仓库 `tests/` 已内置 140+ 用例）。

---

## 下一步

- 完整的语言参考 → [manual.md](./manual.md)
- 工具链构建与故障排查 → [../toolchains/README.md](../toolchains/README.md)、[../toolchains/docs/REBUILD.md](../toolchains/docs/REBUILD.md)
- 跨语言性能对比 → [../examples/projects/benchmarks/README.md](../examples/projects/benchmarks/README.md)
