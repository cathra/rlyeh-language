# Rlyeh 标准库速查（v0.1.0）

> 实际布局（2026-08-22 模块化拆分）：`rlyeh-std/rlyeh/module.rl` 根模块（String / Vec / HashMap / Option /
> Result + 全部 extern 声明 + `module` 声明 + `import` 重新导出）+ 子模块文件 `time.rl` / `io.rl` /
> `net.rl` / `sync.rl`。**裸名即用**（`import time::Duration;` 等已在根模块重导出）。
> 完整 API 与目标架构见 `docs/std-lib.md`；本速查只列 MVP 已实现（✅）项，编写代码可用。

## 内建（无需导入）

- `println(expr)` / `print(expr)`：0–1 参数，自动按类型输出，**无 `{}` 占位符**
- 长度：`.len` 字段或 `.len()` 方法（String / Vec / HashMap；**无内建 `len` 函数**）

## String（✅）

```rlyeh
let s = String::from("hello");
let t = s + String::from(" world");   // 拼接：深拷贝
s.push_str("!");
s.clone();
s.len();
let ch = s[0];
let sub = s[1..<4];              // substring → 新 String
s == t; s < t;
s.contains("Rlyeh"); s.starts_with("Hello");
s.replace("Hello", "Hi"); s.to_upper(); s.to_lower(); s.trim();
s.split(",");                    // Vec<String>
s.repeat(3); s.strip_prefix("Hello");  // Option<String>
s.truncate(5);
int_to_string(42); string_to_int("42");
let r = s.as_str();              // &str 只读视图（零拷贝）
```

> `str` 字面量值参与操作自动升级为 String；`String` 形参位置字面量实参自动升级。

## Vec<T>（✅）

```rlyeh
let v: Vec<i64> = Vec::new();
v.push(10); v.push(20);
let n = v.len();
let x = v[0];
let part = v[1..<3];            // 动态切片 → 全新缓冲
let f = v.first();              // Option<T>
let l = v.last();
v.pop();                        // Option<T>
v.get(2);                       // Option<T>
v.iter();                       // 瘦指针迭代器（V1）
v.iter_mut();
v.reverse(); v.swap(0,1);
v.sort(); v.is_empty(); v.with_capacity(8);
let idx = v.binary_search(&30);
v.as_slice();                   // &[i64]（零拷贝）
v.as_mut_slice();               // &mut [i64]
let sum = v.iter().map(|x| x*2).collect();  // 适配器（V1）
```

## HashMap<K, V>（✅）

```rlyeh
let m: HashMap<String, i64> = HashMap::new();
m.insert(String::from("a"), 1);
let v = m.get(String::from("a"));        // Option<V>
m.remove(String::from("a"));
m.contains_key(String::from("a"));
m.iter_pairs();                          // 键值对迭代
m.with_capacity(8);
```

## Option<T> / Result<T, E>（✅）

```rlyeh
let o = Some(42);
o.unwrap_or(0);
o.unwrap();
o.map(|x| x+1);
o.and_then(|x| Some(x+1));
let r = Result::Err("boom");
r.unwrap_or(100);
match o { Some(x) => x, None => 0 }
match r { Ok(v) => v, Err(e) => -1 }
```

> `?` 运算符（K1 ✅）：`expr?` 在 Option/Result 上下文早返回。

## 切片引用 `&[T]` / `&mut [T]`（✅ S1/S2/S3）

零拷贝胖指针视图（`{ data, len }`）。`&[T; N]` 经 unsize coercion → `&[T]`；方法 `.len()`/`.first()`/`.last()`/`.iter()`/`.as_ptr()`；`Vec::as_slice()`/`as_mut_slice()`。

## 时间（✅ `time` 子模块）

```rlyeh
let d = Duration { micros: 90000000 };
d.secs(); d.millis(); d.micros(); d.nanos();
let t = Instant::now();
let el = t.elapsed();
```

> `SystemTime` 等见 `docs/std-lib.md`。

## IO（✅ `io` 子模块）

```rlyeh
let s = io::read_file(String::from("a.txt"));
io::write_file(String::from("a.txt"), s);
```

> `File` / `Path` / fs 函数（N1/N3 ✅）、NIO `Poller`/`Interest`/`Event`/`set_nonblocking`（R1/R2 ✅）已实现；详见 `docs/std-lib.md`。

## 网络（✅ `net` 子模块）

`TcpStream` / `TcpListener` / `SocketAddr`（O1/O2）、`HttpClient::get/post`（O3 + Y3 连接复用）、`UdpSocket`（Y7）、字节序与主机名查询已实现；WASI 下网络禁用（L4 ✅）。

## 同步（✅ `sync` 子模块）

`Mutex` / `RwLock`（pthread 锁）、`Condvar` / `Barrier` / `Channel`（P1–P3 ✅）、`Thread::start`/`join`/`sleep`/`join_all`/`Builder`（S0/S2/Y8）。

## 序列化（✅ Q1/Q2/Q4）

- JSON：`json::stringify(v)` / `json::parse::<T>(s)`（L2 ✅）；泛型入口 `json::to_string`/`json::from_str`/`json::to_writer`/`json::from_reader`（Q2 ✅）
- TOML：`toml::to_string(v)` / `toml::from_str::<T>(s)`（Q4 ✅）
- `#[derive(Serialize, Deserialize)]`（Q1 ✅）：编译器自动生成 `Serialize`/`Deserialize` 实现

## 异步运行时（✅ S1/W1–W5）

`Future` / `Poll` / `block_on` / `async fn` 状态机（S1）；`Future::poll` 泛型化 `type Output` + `cx`（W1）；控制流内 await 展开（W2）；`future::join_all` / `timeout` / `sleep`（W4/W3）；`recv_async`/`get_async`/`post_async`（W5）。

## 智能指针（✅ K2/K3/K4）

`Box<T>` / `Rc<T>` / `Arc<T>` / `Gc<T>`（含 `gc_region` 保守标记-清除）。

## 迭代器

`for` 接入数组 / Vec / HashMap / 自定义（`next() -> Option<T>`）；适配器 `map`/`filter`/`fold`/`collect`/`take`/`skip`（J1–J3 ✅）。`Vec::iter()`/`iter_mut()` 瘦指针迭代器（V1）、`String::chars()`/`lines()`（V2）。
