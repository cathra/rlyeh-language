# 11. 标准库参考

Rlyeh 标准库采用模块化拆分（2026-08-22 落地）：`rlyeh-std/rlyeh/core.rl` 根模块 + 子模块 `time.rl` / `io.rl` / `net.rl` / `sync.rl`。**裸名即用**（`import time::Duration;` 等已在根模块重导出）。

各类型 / 成员 / 方法的完整说明与用例见 [标准库详述](./std/index.md)：

| 类型 | 详述文档 |
|------|----------|
| `String` | [std/string.md](./std/string.md) |
| `Vec<T>` | [std/vec.md](./std/vec.md) |
| `HashMap<K,V>` | [std/hashmap.md](./std/hashmap.md) |
| `Option<T>` | [std/option.md](./std/option.md) |
| `Result<T,E>` | [std/result.md](./std/result.md) |
| `File` / `Path` / fs | [std/io.md](./std/io.md) |
| `TcpStream` / `TcpListener` / `HttpClient` / `UdpSocket` | [std/net.md](./std/net.md) |
| `Mutex` / `RwLock` / `Condvar` / `Channel` / `Thread` | [std/sync.md](./std/sync.md) |
| `Duration` / `Instant` / `SystemTime` | [std/time.md](./std/time.md) |
| `json` / `toml` / `#[derive(Serialize, Deserialize)]` | [std/json.md](./std/json.md) / [std/toml.md](./std/toml.md) |
| `Future` / `Poll` / `Context` | [std/future.md](./std/future.md) |
| `Box` / `Rc` / `Arc` / `Gc` | [std/smart-pointers.md](./std/smart-pointers.md) |

## 11.1 内建（无需导入）

- `println(expr)` / `print(expr)`：0–1 参数，自动按类型输出，**无 `{}` 占位符**
- 长度：`.len` 字段或 `.len()` 方法（String / Vec / HashMap；**无内建 `len` 函数**）

## 11.2 打印（std-lib §1）

`println(expr)` / `print(expr)`：0–1 参数，自动按类型输出。

## 11.3 String（std-lib §3）

`String::from(s)` / `s + t`（深拷贝）/ `s.push_str` / `s.clone` / `s.len` / `s[0]` / `s[lo..<hi]`（substring）/ `==` `<` 比较 / `contains` / `starts_with` / `replace` / `to_upper` / `to_lower` / `trim` / `split` / `repeat` / `strip_prefix` / `truncate` / `int_to_string` / `string_to_int` / `s.as_str()`（零拷贝视图）。

## 11.4 Vec（std-lib §4）

`Vec::new` / `push` / `len` / `[]` / `v[lo..<hi]`（动态切片）/ `first` / `last` / `pop` / `get` / `iter` / `iter_mut` / `reverse` / `swap` / `sort` / `is_empty` / `with_capacity` / `binary_search` / `as_slice` / `as_mut_slice`。

## 11.5 HashMap（std-lib §5）

`HashMap::new` / `insert` / `get` / `remove` / `contains_key` / `iter_pairs` / `with_capacity`。

## 11.6 Option / Result（std-lib §6）

`Some`/`None`/`Ok`/`Err`；`unwrap_or` / `unwrap` / `map` / `and_then`；`?` 运算符（K1）。

## 11.7 切片（std-lib §7）

`&[T]` / `&mut [T]`：零拷贝胖指针视图（S1/S2/S3）；动态切片 `v[lo..<hi]`（值拷贝）与之区分。

## 11.8 位运算（std-lib §8）

内建 `& | ^ << >> ~`；示例见附录。

## 11.9 序列化

- `json::stringify(v)` / `json::to_string(v)`：标量 / String / `&str` / 数组 / struct / Vec / HashMap（键序确定性）
- `json::parse::<T>(s)` / `json::from_str::<T>(s)`：`i64` / `bool` / `String` / `HashMap<K,V>`（MVP 直接返回 T，非法输入给默认值）
- `json::to_writer(w, v)` / `json::from_reader::<T>(r)`：流式（Q2 ✅）
- `toml::to_string(v)` / `toml::from_str::<T>(s)`（Q4 ✅）
- `#[derive(Serialize, Deserialize)]`：自动生成编解码（Q1 ✅）

## 11.10 动态切片（std-lib §7 / 语言速查）

`v[lo..<hi]`：返回全新缓冲（值拷贝），越界 clamp 到 `[0, len]`，`start>=end` 返回空。

---

## 更多示例

`HashMap.get` 返回 `Option`（强制处理"无此键"）：

```rlyeh
let m: HashMap<String, i64> = HashMap::new();
m.insert(String::from("a"), 1);
let v = m.get(String::from("a"));
match v {
    Some(x) => println(x),
    None => println(0),
};
```

可运行版本见 [`examples/by-chapter/10-stdlib.rl`](../../examples/by-chapter/10-stdlib.rl)；各类型完整成员/方法见 [std/index.md](./std/index.md)。

---

[← 上一章：并发模型](./10-concurrency.md) | [返回手册目录](./index.md) | [下一章：编译器与构建 →](./12-compiler-build.md)
