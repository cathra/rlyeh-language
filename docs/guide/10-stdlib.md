# 10. 标准库

Rlyeh 标准库采用**模块化拆分**（2026-08-22 落地）：`rlyeh-std/rlyeh/core.rl` 根模块（String / Vec / HashMap / Option / Result + 全部 extern 声明 + `module` 声明 + `import` 重新导出）+ 子模块文件 `time.rl` / `io.rl` / `net.rl` / `sync.rl`。裸名即用（`import time::Duration;` 等已在根模块重导出）。

> 每个类型的**完整成员、方法、签名与用例**见 [语言手册 · 标准库详述](../manual/std/index.md)。

## 10.1 内建打印（无需导入）

`println(expr)` / `print(expr)`：0–1 参数，自动按类型输出，**无 `{}` 占位符**（与 Rust 不同）。

```rlyeh
println("Hello");
println(42);
println(true);
```

- 长度：`.len` 字段或 `.len()` 方法（String / Vec / HashMap；**无内建 `len` 函数**）

## 10.2 String

```rlyeh
let s = String::from("Hello");
let t = s + String::from(" World");   // 拼接（深拷贝）
println(s.len());                     // 5
```

详见 [manual/std/string.md](../manual/std/string.md)。

## 10.3 Vec

```rlyeh
let v: Vec<i64> = Vec::new();
v.push(10);
v.push(20);
println(v.len());                     // 2
println(v[0]);                        // 10
let part = v[1..<3];                  // 动态切片 → 全新缓冲
```

详见 [manual/std/vec.md](../manual/std/vec.md)。

## 10.4 HashMap

```rlyeh
let m: HashMap<String, i64> = HashMap::new();
m.insert(String::from("a"), 1);
let v = m.get(String::from("a"));     // Option<i64>
```

详见 [manual/std/hashmap.md](../manual/std/hashmap.md)。

## 10.5 Option / Result（错误处理）

```rlyeh
let o = Some(42);
let v = o.unwrap_or(0);
let r = Result::Ok(10);
match r {
    Result::Ok(x) => println(x),
    Result::Err(e) => println(e),
}
```

`?` 运算符（见 §3.2）在 Option/Result 上下文做早返回。详见 [manual/std/option.md](../manual/std/option.md) / [result.md](../manual/std/result.md)。

## 10.6 其它标准库模块

- **时间** `time`：`Duration` / `Instant` / `SystemTime` —— [manual/std/time.md](../manual/std/time.md)
- **IO** `io`：`read_file` / `write_file` / `File` / `Path` / fs —— [manual/std/io.md](../manual/std/io.md)
- **网络** `net`：`TcpStream` / `TcpListener` / `HttpClient` / `UdpSocket` —— [manual/std/net.md](../manual/std/net.md)
- **同步** `sync`：`Mutex` / `RwLock` / `Condvar` / `Channel` / `Thread` —— [manual/std/sync.md](../manual/std/sync.md)
- **序列化**：JSON（`json::*`）、TOML（`toml::*`）、`#[derive(Serialize, Deserialize)]` —— [manual/std/json.md](../manual/std/json.md) / [toml.md](../manual/std/toml.md)
- **异步运行时**：`Future` / `Poll` / `block_on` / `future::*` —— [manual/std/future.md](../manual/std/future.md)
- **智能指针**：`Box` / `Rc` / `Arc` / `Gc` —— [manual/std/smart-pointers.md](../manual/std/smart-pointers.md)

---

[← 上一章：Actor 并发](./09-actors.md) | [返回指南目录](./index.md) | [下一章：编译目标与工具链 →](./11-targets-toolchain.md)
