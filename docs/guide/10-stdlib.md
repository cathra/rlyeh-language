# 10. 标准库

> 本章目标：概览 Rlyeh 标准库提供的核心类型与模块，知道"什么场景用什么"，并能写出基本可用的代码。每个类型的**完整成员、方法签名、边界条件与用例**见 [语言手册 · 标准库详述](../manual/std/index.md)（那里逐一讲解）。
>
> **标准库采用模块化拆分**：根模块 `rlyeh-std/rlyeh/core.rl` 提供 `String` / `Vec` / `HashMap` / `Option` / `Result` 及全部 `extern` 声明，并重新导出子模块（`time` / `io` / `net` / `sync` 等），所以**裸名就能用**，不必层层 `import`。

---

## 10.1 内建打印（无需导入）

`println(expr)` / `print(expr)` 是内建宏，**自动按类型格式化**，**没有 `{}` 占位符**（和 Rust 不同）：

```rlyeh
println("Hello");
println(42);
println(true);
println(3.14);
```

> **C 程序员的视角**：`println(x)` ≈ `printf("%...\n", x)`，但 Rlyeh 替你根据 `x` 的类型选格式符，不用记 `%d`/`%ld`/`%f`/`%s`，也杜绝了"占位符和参数类型不匹配"这类 C 经典未定义行为。

- 多值：`println("x=", x, " y=", y)`（逗号分隔，自动拼接）。
- 长度获取：容器用 `.len()` 方法或 `.len` 字段（String / Vec / HashMap）；**没有全局 `len()` 函数**。

---

## 10.2 String（字符串）

Rlyeh 的 `String` 是**拥有所有权的、可增长的 UTF-8 字节串**（类似 C++ `std::string`、Rust `String`），不是 C 的 `char*`。

```rlyeh
let s = String::from("Hello");
let t = s + String::from(" World");   // 拼接（产生新 String，深拷贝）
println(s.len());                     // 5：字节长度
println(s + "!");                     // 字面量 "!" 在 String 形参位置自动升级为 String
```

> **C 程序员的视角**：C 里字符串是 `char*` + `\0` 结尾，拼接要 `strcat`/`snprintf`（容易缓冲区溢出）。Rlyeh 的 `String` **自动管理长度与容量、自动扩容、离开作用域自动释放**，不会溢出也不会泄漏。
>
> **`str` 值一等类型**：字符串字面量（`"abc"`）和绑定了字面量的变量，在调用方法 / `+` 拼接 / 比较时会**自动升级为 `String`**——你不用手动 `String::from`。但 `String` 形参位置的字面量实参会**自动升级**，所以 `m.push_str("!")` / `f("hi")` 直接可用。

常用方法：`len` / `push_str` / `clone` / `as_str` / `to_upper` / `to_lower` / `trim` / `split` / `replace` / `repeat` / `chars` / `lines` / `starts_with` / `contains` / `strip_prefix` / `strip_suffix` / `truncate` / `int_to_string` / `string_to_int` / `chars` 等。详见 [manual/std/string.md](../manual/std/string.md)。

---

## 10.3 Vec（动态数组）

运行时可变长度的连续容器（见 [§6.2](../guide/06-arrays-slices.md)）：

```rlyeh
let v: Vec<i64> = Vec::new();
v.push(10);
v.push(20);
println(v.len());                     // 2
println(v[0]);                        // 10
let part = v[1..<3];                  // 动态切片 → 全新缓冲（值拷贝）
v.iter();                             // 遍历（V1 ✅）
```

> **C 对比**：`Vec` ≈ 你手写的 `struct { T* ptr; size_t len; size_t cap; }` + `realloc` 增长 + 析构释放，但全自动。详见 [manual/std/vec.md](../manual/std/vec.md)。

---

## 10.4 HashMap（哈希表）

键值映射（类似 C++ `std::unordered_map`、C 里要自己实现或借第三方库）：

```rlyeh
let m: HashMap<String, i64> = HashMap::new();
m.insert(String::from("a"), 1);
let v = m.get(String::from("a"));     // 返回 Option<i64>（键不存在则为 None）
```

> **为什么 `get` 返回 `Option`**：C 里 `hash_lookup` 找不到通常返回 `NULL` 或特殊值，调用方容易忘检查。Rlyeh 用 `Option` 把"可能没有"编码进类型，`match` 或 `unwrap_or` 强制你处理"无此键"的情况。详见 [manual/std/hashmap.md](../manual/std/hashmap.md)。

---

## 10.5 Option / Result（错误处理）

这是 Rlyeh 表达"可能为空 / 可能失败"的两个核心枚举（见 [§5.2](../guide/05-aggregates-generics.md)）：

```rlyeh
// Option<T>：可能有值
let o = Some(42);
let v = o.unwrap_or(0);              // 有则取之，无则用默认值 0

// Result<T, E>：成功或失败
let r = Result::Ok(10);
match r {
    Result::Ok(x) => println(x),
    Result::Err(e) => println(e),
}
```

> **C 程序员的视角**：C 里"函数可能失败"靠返回 `-1` / `NULL` / 设置 `errno`，调用方**可能忘记检查**（于是用了一个无效值，后续崩溃）。Rlyeh 把"可能失败"写进**返回类型 `Result`**：你不 `match` 处理 `Err` 分支，编译器就**不让**你拿里面的成功值。配合 `?` 运算符（[§3.2](../guide/03-basic-syntax.md)）还能像异常一样"向上冒泡"错误，但完全基于返回值、零隐藏控制流。
>
> 完整方法见 [manual/std/option.md](../manual/std/option.md) / [result.md](../manual/std/result.md)。

---

## 10.6 其它标准库模块

| 模块 | 提供 | 用途 | 详述 |
|------|------|------|------|
| `time` | `Duration` / `Instant` / `SystemTime` | 计时、延时、时间点 | [manual/std/time.md](../manual/std/time.md) |
| `io` | `read_file` / `write_file` / `File` / `Path` / fs | 文件与路径操作 | [manual/std/io.md](../manual/std/io.md) |
| `net` | `TcpStream` / `TcpListener` / `HttpClient` / `UdpSocket` | 网络编程 | [manual/std/net.md](../manual/std/net.md) |
| `sync` | `Mutex` / `RwLock` / `Condvar` / `Channel` / `Thread` | 线程同步原语 | [manual/std/sync.md](../manual/std/sync.md) |
| 序列化 | `json::*` / `toml::*` / `#[derive(Serialize, Deserialize)]` | 结构化数据编解码 | [manual/std/json.md](../manual/std/json.md) / [toml.md](../manual/std/toml.md) |
| 异步 | `Future` / `Poll` / `block_on` / `future::*` | 异步运行时 | [manual/std/future.md](../manual/std/future.md) |
| 智能指针 | `Box` / `Rc` / `Arc` / `Gc` | 堆分配与共享（见 [§8.5/§8.6](../guide/08-memory.md)） | [manual/std/smart-pointers.md](../manual/std/smart-pointers.md) |

> 裸名即用：例如 `import time::Duration;` 已在根模块重导出，多数情况下直接写 `time::Duration` 即可。

---

## 10.7 综合示例：读文件并统计行数

```rlyeh
fn main() {
    let content = io::read_file("data.txt");   // 读取整个文件为 String
    let lines = content.lines();               // 按行切分（返回可遍历的视图）
    let mut count = 0;
    for _l in lines {
        count += 1;
    }
    println("行数：", count);
}
```

> 这只是示意 API 形态；具体方法名/签名以 [manual/std/io.md](../manual/std/io.md) 为准。

---

## 练习

1. 运行 [`examples/by-chapter/10-stdlib.rl`](../../examples/by-chapter/10-stdlib.rl)，统计一个字符串里各字符出现的次数（用 `HashMap`）。
2. 用 `Option.unwrap_or` 处理"键不存在"的情况，对比 C 里 `NULL` 返回值的隐患。
3. 用 `#[derive(Serialize, Deserialize)]` 把一个 struct 序列化成 TOML 再 `from_str` 读回来，验证往返一致。

---

[← 上一章：Actor 并发](./09-actors.md) | [返回指南目录](./index.md) | [下一章：编译目标与工具链 →](./11-targets-toolchain.md)
