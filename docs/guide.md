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
```

`match` 用于枚举解构（见 §5.2）。

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

> **MVP 状态**：L0 仅实现了**移动语义**（值拷贝/移动）与**方法接收者** `&self` / `&mut self`。
> `&x` 引用表达式、`&T` 参数类型、解引用 `*` 在 MVP 阶段**未实现**（typecheck 显式报 Unsupported），属规划特性。

```zeta
let s = String::from("hello");
let t = s;               // 移动：s 语义上不可再用（值拷贝语义）
// let r = &t;           // MVP 不支持 & 表达式（规划）
```

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
```

### 8.3 L2 / L3

`Rc<T>` / `Arc<T>`（引用计数）与 `Gc<T>`（可选 GC）为规划中特性，MVP 未实现。

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

```zeta
let s = String::from("Hello, Zeta");
s.len                       // 字节长度
s[0]                        // 按字节索引
s.substring(0, 5)           // "Hello"
s.contains("Zeta")          // true
s.starts_with("Hello")      // true
s.replace("Hello", "Hi")    // "Hi, Zeta"
s.to_upper() / s.to_lower() / s.trim()
s.split(",")                // Vec<String>
s.repeat(3)
s.strip_prefix("Hello")     // Option<String>
s.truncate(5)
int_to_string(42)           // "42"
string_to_int("42")         // 42
```

### 10.3 Vec

```zeta
let mut v = Vec::new();
v.push(10); v.push(20); v.push(30);
v.len / v.is_empty / v.cap
v[0] / v.get(i) / v.set(i, x)
v.pop()                     // Option<T>
v.contains(20)              // bool
v.find(20)                  // 下标，未找到 -1
v.remove(i) / v.insert(i, x)
v.sort() / v.reverse() / v.swap(i, j)
v.first() / v.last()        // Option<T>
v.binary_search(20)         // 最左下标，未找到 -1
v.clear()
```

### 10.4 HashMap

```zeta
let mut m = HashMap::new();
m.insert("k", 42);
m.contains_key("k")         // bool
m.get("k")                  // Option<V>
m.remove("k")               // Option<V>
m.len / m.is_empty / m.cap
m.clear()
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
let content = read_file("data.txt");       // 读整个文件；失败返回空串
write_file("out.txt", content);            // 截断写；返回字节数，失败 -1
append_file("log.txt", line);              // 追加；返回字节数，失败 -1
let line = read_line();                    // 从 stdin 读一行（不含换行符）
```

### 10.7 网络

```zeta
let host = hostname();                     // 本机主机名
let pair = socketpair_stream();            // AF_UNIX SOCK_STREAM 全双工 fd 对
send_all(fd, data);                        // 循环发满
let got = recv_some(fd, n);                // 接收（SOCK_STREAM 需循环收满）
let fd = tcp_connect("127.0.0.1", 8080);   // TCP 连接；失败 -1
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
zeta build app.zeta --target wasm32-wasi -o app.wasm
wasmtime app.wasm      # WASI preview1 运行时运行
```

依赖：`wasi-libc` sysroot（`brew install wasi-libc` 或 `WASI_SYSROOT`）、`wasm-ld`（`brew install lld`）。入口链：WASI `_start`（crt1）→ Zeta `main()`。未使用的 extern 符号（socket 等 WASI 无对应）不会引入链接错误。

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

### 已知限制（MVP）

**规划中 / 未实现**：
- **宏系统**：`println!` / `vec!` / `format!` 等宏调用不支持（`!` 是 `not` 一元运算符）；打印用内建 `println(expr)`（0–1 参数，无 `{}` 格式化）。
- **引用与借用**：`&x` 表达式、`&T` 参数类型、`str` 类型、解引用 `*`、裸指针均未实现；仅方法接收者 `&self` / `&mut self` 可用。
- **闭包**：`|x| x + 1` 语法可解析，typecheck 报 Unsupported（规划）。
- **运算符**：`?` 错误传播、`dyn Trait`、函数指针未实现。
- **所有权层级**：L2 `Rc<T>` / `Arc<T>`、L3 `Gc<T>` 未实现（规划）。
- **并发**：`serde` / `fmt` / `async` 模块为规划；actor 的 `async` 方法 + `.await` + `send` 已实现（见 §9）。
- **迭代器协议**：`for i in 0..<10` 数值区间可用；`Iterator` trait / `collect` 未实现。

**实现约束**：
- **std 模块化**：标准库位于 `zeta-std/zeta/`，`core.zeta` 根模块（String / Vec / HashMap / Option / Result + extern 集中声明）拆分为 `time` / `io` / `net` / `sync` 四个子模块文件，driver 加载时模块展开 + `use` 重新导出，用户侧裸名即用。
- **net**：`tcp_connect` 依赖平台特定 `sockaddr_in4` 布局（已由 `__zeta_target_os` 双布局化）；WASI 下网络不可用。
- **Actor 运行时**：交叉编译 / WASM 目标下 actor 程序暂不支持（staticlib 为主机架构）。
- **`String::from(s)`**：支持字符串字面量及绑定字面量的变量；非字面量 Str（运行期内容）长度表达未实现。
- **region 选项**：`adaptive` / `with_size (N)` 可用；`strategy (bump)` 等其余选项规划中。
