# Zeta 语言手册（Language Manual）

> 版本：v2.0（MVP）
> 本文为 Zeta 语言的**完整参考手册**：词法、类型系统、表达式、控制流、模块、泛型、内存模型、并发模型、标准库、编译器与工具链。
> 初学者请先阅读 [tutorial.md](./tutorial.md)。权威规范：`grammar.md`（EBNF）/ `semantics.md` / `memory-model.md` / `actor-model.md` / `std-lib.md`。

---

## 目录

1. [语言概述](#1-语言概述)
2. [词法与字面量](#2-词法与字面量)
3. [类型系统](#3-类型系统)
4. [表达式与运算符](#4-表达式与运算符)
5. [语句与控制流](#5-语句与控制流)
6. [函数与闭包](#6-函数与闭包)
7. [模块与可见性](#7-模块与可见性)
8. [泛型与单态化](#8-泛型与单态化)
9. [内存模型](#9-内存模型)
10. [并发模型](#10-并发模型)
11. [标准库参考](#11-标准库参考)
12. [编译器与构建](#12-编译器与构建)
13. [工具链手册](#13-工具链手册)
14. [已知限制与规划](#14-已知限制与规划)

---

## 1. 语言概述

Zeta 是系统级编程语言，内存安全与零 GC 默认、并发一等公民、数学式语法。编译器为 Rust bootstrap 阶段，代码生成走 LLVM IR，经 clang 汇编链接为原生可执行文件或 WASM。

| 维度 | 设计 |
|------|------|
| 内存 | 分层模型：L0 所有权 / L1 区域 / L2 Rc / L3 GC |
| 并发 | `actor` 语言级构造 + 监督重启；`async fn` 状态机 |
| 泛型 | 单态化（每具体类型实例化一份代码） |
| 编译 | 增量编译（模块级缓存）、PGO profile、WASM 交叉编译 |
| FFI | `extern fn` 声明 libc 符号 |

---

## 2. 词法与字面量

### 2.1 注释

```zeta
// 行注释
/// 文档注释（zeta doc 提取生成 Markdown）
```

### 2.2 数字字面量

- 整型：`42`、`0xFF`（十六进制）、`0b1010`（二进制）
- 浮点：`3.14`、`1e6`

### 2.3 字符串与原始字符串

```zeta
let s = "hello";
let raw = r#"C:\path\no\escape"#;   // 带哈希原始字符串（r# / r## ...）
```

### 2.4 时间字面量

```zeta
9am    6pm    10pm    12am    3pm    // 时间字面量（比较链 / 区间判断用）
```

### 2.5 标识符

普通标识符（字母/下划线开头）；`r#ident` 原始标识符（关键字作标识符）。

---

## 3. 类型系统

### 3.1 标量类型

| 类型 | 说明 |
|------|------|
| `i8/i16/i32/i64/isize` | 有符号整型（字面量默认 `i64`） |
| `u8/u16/u32/u64/usize` | 无符号整型 |
| `f32/f64` | 浮点 |
| `bool` | 布尔 |
| `char` | 字符 |
| `()` | 单元类型 |

### 3.2 聚合类型

- **struct**：`struct Point { x: i64, y: i64 }`，字面量构造 `Point { x: 1, y: 2 }`
- **enum**：元组变体 `Circle(f64)` 与结构体变体 `Rect { w, h }`；`match` 解构支持嵌套泛型载荷
- **数组**：`[T; N]`，长度编译期已知；字面量 `[1, 2, 3]`
- **元组**：`(a, b)`（元素多槽）

### 3.3 泛型

`Vec<T>`、`HashMap<K, V>`、`Option<T>`、`Result<T, E>` 等；用户自定义泛型按单态化编译。`json::parse::<T>(s)` 用 **turbofish** 语法 `::<T>` 指定泛型实参。

### 3.4 trait

```zeta
trait Area {
    fn area(&self) -> f64;
}
impl Area for Shape {
    fn area(&self) -> f64 { /* ... */ }
}
```

`dyn Trait`：trait 对象（2 槽胖指针 = 数据指针 + vtable），`&T` 强制转换，方法经 vtable 间接分派。MVP 限制：trait/impl 须非泛型，含 `Self` 签名的方法不可经 dyn 调用。

### 3.5 函数类型

`fn(i64, i64) -> i64` 函数指针类型；函数值可绑定、传参、返回。

### 3.6 引用与指针

| 类型 | 说明 |
|------|------|
| `&T` / `&mut T` | 不可变 / 可变引用；`&x` / `&mut x` 表达式；`*` 解引用；字段访问/方法调用自动剥引用层 |
| `&str` | String 只读借用视图（`s.as_str()`，零拷贝） |
| `*const T` / `*mut T` | 裸指针（G3，与 `&T` 互视） |
| `Box<T>` / `Rc<T>` / `Arc<T>` / `Gc<T>` | 智能指针（见 §9） |

### 3.7 字符串类型

- `String`：堆缓冲（data/len/cap 3 槽），O(n) 扩容
- `str`（字面量值）：运行时为指向静态数据的 `i8*`，参与操作自动升级为 String
- `&str`：只读借用视图，支持 `len()` / `r[i]` / `substring`

---

## 4. 表达式与运算符

### 4.1 算术与位运算

```zeta
+  -  *  /  %           // 算术
&  |  ^  <<  >>         // 位运算
&&  ||  !              // 逻辑
==  !=  <  <=  >  >=    // 比较
```

**优先级**：`*` > `+` > `<<` > `&` > `^` > `|`；比较 `>` 位运算。裸 `x & 3 == 2` 会按 bool 处理，需写 `(x & 3) == 2`。

### 4.2 数学式比较链

```zeta
0 < x < 10        // 0 < x && x < 10（区间内 = 交集）
0 > x > 10        // x < 0 || x > 10（区间外 = 并集）
0 <= x <= 10      // 0 <= x && x <= 10
```

### 4.3 集合判断 `in`

```zeta
x in (1, 3, 5)              // x == 1 || x == 3 || x == 5
x in 0..<10                 // 裸范围 = 区间判断 [0, 10)
x in 0...10                 // [0, 10]
x in 0<..10                 // (0, 10]
x not in (0..<10)           // !(x in ...)
hour in (9am...6pm)         // 时间字面量区间
```

### 4.4 `?` 错误传播

```zeta
fn chain(a: i64, b: i64) -> Option<i64> {
    let q = try_div(a, b)?;     // Some 解包；None 则 return None
    Some(q + 1)
}
```

- `Option<T>`：desugar 为 `match expr { Some(v) => v, None => return None }`
- `Result<T, E>`：desugar 为 `match expr { Ok(v) => v, Err(e) => return Err(e) }`

### 4.5 `as` 类型转换

```zeta
expr as Type     // 显式类型转换（如 i64 as f64）
```

### 4.6 迭代器适配器

```zeta
[1, 2, 3].map(|x| x * 2)                 // Vec: [2, 4, 6]
[1, 2, 3, 4, 5].filter(|x| x % 2 == 1)   // Vec: [1, 3, 5]
[1, 2, 3, 4].fold(0, |acc, x| acc + x)   // 10
[7, 8, 9].collect()                      // Vec: [7, 8, 9]
[1, 2, 3, 4, 5].take(3) / .skip(2)
Counter::new(6).filter(|x| x > 1).map(|x| x * x)   // 链式
```

自定义迭代器：类型存在 `next() -> Option<T>` 方法即可接入 `for`。MVP：适配器参数须无捕获闭包，返回 `Vec<T>`（急切求值）。

---

## 5. 语句与控制流

### 5.1 绑定

```zeta
let x = 6;              // 不可变绑定（类型推断）
let mut v = Vec::new(); // 可变绑定
let ref rz = z;         // ref 绑定：引用而非拷贝
```

### 5.2 条件

```zeta
if cond { ... } else { ... }
if 0 < x < 10 { ... }   // 数学式比较链
```

### 5.3 循环

```zeta
while cond { ... }
loop { break; continue; }
for i in 0..<10 { ... }     // 半开区间 [0, 10)
for j in 1...3 { ... }      // 双闭区间 [1, 3]
for x in vec { ... }        // Vec 迭代
for (k, v) in map { ... }   // HashMap 迭代
for x in arr { ... }        // 数组迭代（索引遍历）
```

### 5.4 match

```zeta
match s {
    Shape::Circle(r) => 3.14 * r * r,
    Shape::Rect { w, h } => w * h,
    _ => 0,
}
```

支持 `ref` / `ref mut` 子模式、嵌套泛型载荷解构。MVP 注意：match 先把匹配值拷贝到临时槽，`ref` 绑定指向该拷贝。

### 5.5 函数定义

```zeta
fn add(a: i64, b: i64) -> i64 { a + b }   // 末表达式为返回值
fn main() {}
```

---

## 6. 函数与闭包

### 6.1 函数一等值（H1）

```zeta
let f = add;                    // 函数值绑定
let g: fn(i64, i64) -> i64 = add;
f(3, 4) / apply(add, 10, 20)
```

### 6.2 闭包

| 形态 | 语法 | 说明 |
|------|------|------|
| 无捕获闭包（H2） | `\|x, y\| expr` | desugar 为匿名函数 + 函数指针，需 fn 类型上下文 |
| 捕获闭包 IIFE（H3） | `(\|x\| expr)(args)` | 立即调用，外层变量按值捕获 |
| 闭包值对象（H5） | `let f = \|x: i64\| expr;` | 绑定后反复调用；无注解参数由首次调用点实参推断 |

MVP 约束：参数模式仅简单标识符与 `_`；捕获闭包值不跨函数边界（作 fn 实参/返回值报错）；不支持嵌套捕获闭包；按引用捕获与 `move` 规划中。

### 6.3 async fn

```zeta
async fn get_value(x: i64) -> i64 { x * 2 }

fn main() {
    let mut fut = get_value(21);
    println(block_on(&mut fut));   // 42
}
```

desugar 为 Future 结构体 + poll 状态机；`.await` 挂起/恢复。MVP：参数/返回限 `i64`/`()`，控制流块内 await 不支持。

---

## 7. 模块与可见性

```zeta
module math {
    pub const PI: f64 = 3.14159;
    pub fn square(x: i64) -> i64 { x * x }
    fn hidden() {}          // 私有
}

import math::PI;
import math::square as sq;
module foo;                    // 多文件：foo.zeta / foo/module.zeta
模块名::Enum::Variant          // 跨模块路径（扁平名字空间，路径即「模块名::」）
```

---

## 8. 泛型与单态化

- 标准库泛型：`Vec<T>` / `HashMap<K, V>` / `Option<T>` / `Result<T, E>` / `Rc<T>` 等
- 用户自定义泛型：单态化编译
- `impl<T> Option<T> { ... }` 形式的方法定义
- turbofish：`json::parse::<HashMap<i64, i64>>(s)`
- MVP：trait 泛型、`#[derive]` 宏规划中

---

## 9. 内存模型

### 9.1 L0 所有权与借用

- 值语义：赋值/传参默认拷贝（聚合为对象指针拷贝，共享底层缓冲）
- 借用规则（NLL 近似）：
  - 多个 `&` 共享借用可共存
  - `&mut T` 与任何活跃借用互斥（`BorrowConflict`）
  - `&mut` 要求 `let mut` 绑定（`BorrowMutImmutable`）
  - 活跃借用期间直接赋值被借用变量报错；局部引用逃逸报 `DanglingReference`
  - 语义有意宽松：读取被借用变量与 `*p` 写入允许
- `ref` / `ref mut` 模式：绑定为引用而非拷贝

### 9.2 L1 区域（Region）

```zeta
region 'r {
    let data = BigStruct::new() in 'r;   // 区域分配
}   // 批量释放（零开销）

return transfer data out of 'r;          // 所有权移出

region 'r adaptive { ... }               // PGO 画像建议容量
region 's with_size (4096) { ... }       // 精确预分配
region 't strategy (bump) { ... }        // bump 分配策略
```

运行时接线 `zeta-region-alloc` C ABI（`zeta_region_enter/alloc/transfer/exit`）。

### 9.3 L2 智能指针

```zeta
Box::new(x)                 // 1 槽指针；* 解引用；字段/方法自动剥层
Rc::new(x) / r.clone()      // 强计数共享
r.strong_count() / r.weak_count()
r.downgrade()               // → Weak
w.upgrade()                 // → Option<Rc<T>>
r.clone().try_unwrap()      // → Result<T, Rc<T>>
```

MVP：无自动 drop（计数只增不减，与 Vec/String 一致）。

### 9.4 L3 GC

```zeta
gc_region {
    let a = Gc::new(42);    // 块结束触发标记-清除周期
}
let g = gc_region { let inner = Gc::new(100); inner };  // 逃逸对象登记为 root
```

保守标记-清除运行时（`zeta-gc-runtime`），单线程无锁。MVP 限制：块外对象永不回收、stop-the-world 非增量、递归标记。

---

## 10. 并发模型

### 10.1 Actor

```zeta
actor Counter {
    value: i64 = 0,

    pub fn increment(amount: i64) -> i64 {
        self.value += amount;
        self.value
    }
}

let c = Counter::new();          // 或 new_supervised(strategy)
let r = c.increment(10).await;   // ask 同步往返
send c.increment(1);             // fire-and-forget
```

- 消息经「kind 槽 + 3 个 i64 消息槽」传递；邮箱 FIFO 互斥处理
- 崩溃协议：方法返回 `-1` → Panic → ask 返回 0；supervisor 重建重启
- 监督策略：`0=OneForOne` `1=AllForOne` `2=RestartForOne`
- 运行时：工作窃取调度（worker = CPU 核数）；WASI 下为单线程同步模式
- WASI 下 `net` 模块明确禁用

### 10.2 同步原语

| 原语 | API |
|------|-----|
| `Mutex` | `lock()` / `unlock()` / `try_lock()` |
| `RwLock` | `read_lock()` / `write_lock()` / `try_read_lock()` / `try_write_lock()` / `unlock()` |
| `Condvar` / `Barrier` | pthread 封装 |
| `Channel` | `channel()` → ChannelPair + `Sender`/`Receiver` + `recv_async` |

---

## 11. 标准库参考

### 11.1 内建打印

```zeta
println("Hello") / println(42) / println(3.14) / println(true) / println() / print(x)
// 格式化宏：println! / print! / format! / dbg! / eprintln! / eprint!
println!("value = {}", x);      // {} 占位
```

### 11.2 String

`String::from(s)`（字面量 / String / `&str`）、`s.len`、`s[i]`、`substring`、`contains`、`starts_with`、`replace`、`to_upper`、`to_lower`、`trim`、`split(",")`、`repeat`、`strip_prefix`、`truncate`、`as_str`、`int_to_string(i)`、`string_to_int(s)`。

### 11.3 Vec

`Vec::new()`、`with_capacity`、`push`、`pop`、`get(i)`、`set(i,x)`、`contains`、`find`、`remove`、`insert`、`sort`、`reverse`、`swap`、`first`、`last`、`binary_search`、`clear`、`len/cap`、`is_empty()`。

### 11.4 HashMap

`HashMap::new()`、`insert`、`contains_key`、`get`、`remove`、`clear`、`len/cap`、`is_empty()`；`map![k => v, ...]` 宏。

### 11.5 Option / Result

`is_some/is_none/is_ok/is_err`、`unwrap`、`unwrap_or`、`expect`、`map`、`and_then`、`map_err`、`propagate`（≡ `?`）。

### 11.6 文件 IO

`read_file(path)` / `write_file(path, content)` / `append_file(path, line)` → `Result<_, IoError>`；`read_line()` → stdin 读一行；`e.message()`。

### 11.7 网络

`hostname()`、`socketpair_stream()`、`send_all(fd, data)`、`recv_some(fd, n)`、`tcp_connect(port, a, b, c, d)`；NIO：`Poller` / `Interest` / `Event` + `set_nonblocking`；`sendfile`。

### 11.8 时间

`Duration { micros }`（`secs()/millis()/micros()/nanos()`）、`Instant::now()` / `t.elapsed()`。

### 11.9 序列化

- `json::stringify(v)`：标量 / String / `&str` / 数组 / struct / Vec / HashMap（键序确定性）
- `json::parse::<T>(s)`：`i64` / `bool` / `String` / `HashMap<K,V>`（MVP 直接返回 T，非法输入给默认值）
- `toml` 模块（内建）
- MVP：自定义 `Serialize`/`Deserialize` trait 与 `#[derive]` 规划中

### 11.10 位运算与切片

位运算全链路支持（常量折叠 + 运行时）。动态切片 `v[lo..<hi]` / `v[lo...hi]` / `v[lo<..hi]` 返回全新缓冲（越界 clamp）。

---

## 12. 编译器与构建

### 12.1 本机编译

```bash
zeta build hello.zeta -o hello && ./hello
```

### 12.2 交叉编译

```bash
zeta build app.zeta --target x86_64-apple-macosx
zeta build app.zeta --target arm64-apple-macosx
```

平台内建 `__zeta_target_os`：linux=1 / macos=2 / windows=3 / freebsd=4 / 其他=0 / wasi=5。

### 12.3 WebAssembly

```bash
zeta build app.zeta --target wasm32-wasip1 -o app.wasm
wasmtime app.wasm
```

依赖 `wasi-libc` sysroot 与 `wasm-ld`。actor 程序支持 WASI（需先构建 wasm 版 actor 运行时）。

### 12.4 宏系统

- 声明式宏 `macro_rules!`：`$x:expr` / `$x:ident` / `$x:ty` / `$x:tt` + 重复 `*`/`+`/`?`
- 内置宏：`println!` / `print!` / `format!` / `dbg!` / `eprintln!` / `eprint!` / `arr!` / `vec!` / `map!`
- 原始字符串：`r#"..."#`（任意哈希定界）
- 限制：无卫生宏

### 12.5 PGO profile

```bash
zeta profile app.zeta_profile --out report.md    # 画像 → 区域大小预测报告
# zeta build --profile 注入 PGO；region adaptive 据此建议容量
```

### 12.6 FFI

```zeta
extern fn clock() -> i64;
extern fn fopen(path: String, mode: String) -> i64;
```

支持的标量类型：`i8..i64` / `isize` / `u8..u64` / `usize` / `f32` / `f64` / `bool` / `char` / `()` / `&T` / `String`。`String` 参数 ABI 层传 data 指针；`i32` 返回值自动 sext 清洗。

---

## 13. 工具链手册

### 13.1 部署与安装

#### 前置依赖

| 依赖 | 用途 |
|------|------|
| Rust 工具链（≥ 1.75） | 编译器自举构建 |
| LLVM/Clang | 汇编与链接后端 |
| git | 版本信息 |

#### 一键构建

```bash
./toolchains/build.sh                          # 环境检查 → build → test → 发布 → 冒烟 → 归档
./toolchains/build.sh --no-test                # 跳过测试
./toolchains/build.sh --no-install             # 仅构建 + 归档
./toolchains/build.sh --no-tar                 # 不打包
./toolchains/build.sh --prefix /opt/zeta       # 自定义前缀
```

#### 本地发布

```bash
./toolchains/install.sh                        # 发布到 ~/.zeta
ZETA_PREFIX=/opt/zeta ./toolchains/install.sh  # 自定义前缀
```

#### PATH 与环境变量

| 变量 | 默认 | 说明 |
|------|------|------|
| `PATH` | — | 需含 `$HOME/.zeta/bin` |
| `ZETA_STD_PATH` | `~/.zeta/std`（wrapper 自动注入） | 标准库目录（含 core.zeta） |
| `ZETA_PREFIX` | `$HOME/.zeta` | 安装前缀（install.sh 用） |

```bash
export PATH="$HOME/.zeta/bin:$PATH"
zeta --version    # → zeta 0.1.0
```

#### 归档分发

`toolchains/dist/zeta-toolchain-<ver>-<os>-<arch>.tar.gz`：解压到任意目录，`bin` 加入 PATH 即可（wrapper 按 `bin/../std` 相对定位标准库，可重定位）。

### 13.2 编译器命令 `zeta`（全参考）

```
用法:
  zeta run <file.zeta> [--cache-dir <dir>] [--force] [--no-std] [--verbose]
      编译并运行
  zeta build <file.zeta> [-o <out>] [--cache-dir <dir>] [--force] [--no-std]
              [--verbose] [--target <triple>]
      编译为可执行文件（--target 交叉编译 / wasm32-wasi 生成 .wasm）
  zeta test [<tests-dir>]
      运行测试目录用例（compile-pass/compile-fail/run-pass）
  zeta fmt <file.zeta> [--check] [-w|--write] [--indent N]
      格式化代码（默认输出到 stdout）
  zeta check <file.zeta>
      静态分析（未使用变量/恒常条件/冗余比较/不可达代码）
  zeta doc <file.zeta> [--out <file.md>] [--title <标题>]
      提取 /// 注释生成 Markdown 文档
  zeta bench <file.zeta> [-o <out>] [--runs N] [--warmup N]
      编译并基准计时
  zeta new <name> [--lib]
      创建新项目脚手架（Zeta.toml + src/main.zeta 或 lib.zeta）
  zeta publish [--registry <URL>] [--verbose]
      打包发布到 zep 注册表（重复版本拦截）
  zeta lsp
      启动语言服务器（LSP over stdio，诊断推送）
  zeta profile <file.zeta_profile> [--out <report.md>]
      PGO 画像 → 区域大小预测报告
  zeta --version
      版本信息
```

**子命令速查**

| 子命令 | 用途 | 典型用法 |
|--------|------|----------|
| `run` | 编译并运行 | `zeta run main.zeta` |
| `build` | 编译为可执行文件 | `zeta build main.zeta -o app` |
| `test` | 运行测试用例目录 | `zeta test tests/` |
| `fmt` | 格式化代码 | `zeta fmt src/main.zeta -w` |
| `check` | 静态分析 | `zeta check src/main.zeta` |
| `doc` | 从 `///` 注释生成文档 | `zeta doc lib.zeta --out api.md` |
| `bench` | 基准计时 | `zeta bench fib.zeta --runs 5` |
| `new` | 项目脚手架 | `zeta new myapp`（`--lib` 生成 lib.zeta） |
| `publish` | 发布到注册表 | `zeta publish` |
| `lsp` | 语言服务器 | 编辑器集成 |
| `profile` | PGO 画像分析 | `zeta profile app.zeta_profile` |

### 13.3 独立工具（详细使用说明）

| 工具 | 完整用法 | 说明 |
|------|----------|------|
| `zeta-fmt` | `zeta-fmt [--check] [-w\|--write] [--indent N] <file>` | 格式化；`--check` 只检查、`-w` 写回、`--indent` 指定缩进 |
| `zeta-check` | `zeta-check <file>` | 静态分析（未使用变量 / 恒常条件 / 冗余比较 / 不可达代码） |
| `zeta-doc` | `zeta-doc <file.zeta> [--out <file.md>] [--title <标题>]` | 提取 `///` 注释生成 Markdown |
| `zeta-bench` | `zeta-bench <file.zeta \| 可执行文件> [--runs N] [--warmup N] [--out <路径>] [--quiet]` | 基准计时；支持 `.zeta` 源码或已编译可执行文件 |

> 注：`zeta-fmt`/`zeta-check`/`zeta-doc` 接受文件名参数（非 `--help` 风格）；`zeta-bench` 支持 `-h/--help`。

### 13.4 包管理器 `zep`

```
Zep 是 Zeta 语言的包管理器：项目脚手架、依赖解析、注册表发布与构建集成。

Commands:
  new      创建新项目
  init     初始化当前目录为项目
  add      添加依赖（name[@req]，如 foo@^1.0）
  remove   移除依赖
  build    编译项目
  run      编译并运行（程序参数需以 -- 分隔：zep run -- --flag x）
  test     构建并运行测试（tests/*.zeta）
  update   重新解析依赖并更新 Zeta.lock
  publish  打包并发布到注册表
  search   在注册表中搜索包
  clean    清理 target 目录

Options:
      --verbose                详细输出
      --registry <URL>         注册表地址（默认 ~/.zeta/registry）
```

**本地注册表机制**：默认 `file://$HOME/.zeta/registry`；包存储于 `pkgs/<name>-<ver>.tar.gz` + 索引 `index/<name>.json`。指定其他注册表：`zep --registry /path/to/reg` 或 `zeta publish --registry file:///path/to/reg`。

### 13.5 故障排查

| 现象 | 原因 | 解决 |
|------|------|------|
| `缺少 release 产物` | 未先构建 | `cargo build --release` 后重跑 install.sh |
| 模块未找到 | 标准库未正确复制 | 重跑 install.sh（重建 `~/.zeta/std`） |
| `zeta: command not found` | PATH 未配置 | `export PATH="$HOME/.zeta/bin:$PATH"` |
| clang 链接报错 | 无 LLVM/Clang | `xcode-select --install` |
| wasm 目标缺失 | 未装 target | `rustup target add wasm32-wasip1` |
| `zeta build --target wasm` 报 actor 缺失 | wasm actor 运行时未构建 | `cargo build --target wasm32-wasip1 -p zeta-actor-runtime` |

---

## 14. 已知限制与规划

**规划中 / 未实现**：
- 生命周期标注 `<'a>` / `&'a T`：MVP 语法接受后丢弃（宽松检查），严格验证规划中
- 闭包按引用捕获、`move` 所有权语义
- 多线程 GC（全局锁 / 线程局部堆）、增量回收、write barrier
- `fmt` 模块规划；`Serialize`/`Deserialize` trait 与 `#[derive]` 宏
- 泛型 `join_all` / `timeout` / `sync` 并发原语
- `strategy (pool)` 区域分配策略
- 无自动 drop（`Box`/`Rc`/`Vec`/`String` 显式释放语义规划中）

**实现约束**：
- 标准库位于 `zeta-std/zeta/`（core.zeta 根模块 + time/io/net/sync/fs 子目录），driver 模块展开 + import 重导出，用户侧裸名即用
- WASI 下 `net` 模块明确禁用（`__zeta_target_os == 5` 短路）
- 闭包捕获不跨函数边界；trait 非泛型；`HashMap` 键限 i64/String

## 权威文档导航

| 文档 | 内容 |
|------|------|
| [guide.md](./guide.md) | 教程（v2.0，示例导向） |
| [tutorial.md](./tutorial.md) | 教程指南（新手入门，含工具链部署） |
| [grammar.md](./grammar.md) | 完整语法规范（EBNF） |
| [semantics.md](./semantics.md) | 语义规则 |
| [memory-model.md](./memory-model.md) | 分层内存管理规范 |
| [actor-model.md](./actor-model.md) | Actor 并发模型规范 |
| [std-lib.md](./std-lib.md) | 标准库 API 规范 |
| [development-plan.md](./development-plan.md) | 开发计划（阶段 A–F） |
| [mvp-gaps-plan.md](./mvp-gaps-plan.md) | 已知限制消解计划（阶段 G–L） |
| [../toolchains/README.md](../toolchains/README.md) | 工具链构建与发布总览 |
| [../toolchains/docs/REBUILD.md](../toolchains/docs/REBUILD.md) | 重新构建教程 |
| [../toolchains/docs/MANUAL.md](../toolchains/docs/MANUAL.md) | 工具链快速手册 |
