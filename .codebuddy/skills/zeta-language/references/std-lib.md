# Zeta 标准库速查（v0.1.0）

> 实际布局（2026-08-22 模块化拆分）：`zeta-std/zeta/core.zeta` 根模块（String / Vec / HashMap / Option /
> Result + 全部 extern 声明 + `module` 声明 + `import` 重新导出）+ 子模块文件 `time.zeta` / `io.zeta` /
> `net.zeta` / `sync.zeta`（目录形式 `time/module.zeta` 等）。**裸名即用**（`import time::Duration;` 等已在根模块重导出）。
> ⚠️ `docs/std-lib.md` 中的完整 API 为**目标架构（规划）**，MVP 只实现了下列 ✅ 子集。编写代码只可用 ✅ 项。

## 内建（无需导入）

- `println(expr)` / `print(expr)`：0–1 参数，自动按类型输出，**无 `{}` 占位符**
- 长度：`.len` 字段或 `.len()` 方法（String / Vec / HashMap；**无内建 `len` 函数**）

## String（✅ 基本完整）

```zeta
let s = String::from("hello");   // 仅支持字面量 / 绑定字面量的变量
let t = s + String::from(" world");   // 拼接：深拷贝（clone + push_str），结果与左操作数隔离
s.push_str("!");                 // 追加
s.clone();                       // 深拷贝
s.len();                         // 字节长度
let ch = s[0];                   // 按字符索引（步长 1 字节，ASCII 假设）
let sub = s[1..<4];              // substring → 新 String
s == t; s < t;                   // 比较
```

⚠️ 规划（未实现）：`chars` / `split` / `replace` / `to_uppercase` / `trim` / `lines` / `format!`。

## Vec<T>（✅ 基本完整）

```zeta
let v: Vec<i64> = Vec::new();    // new 预分配 cap 4
v.push(10); v.push(20);
let n = v.len();
let x = v[0];                    // 索引读写
let part = v[1..<3];             // 动态切片 → 全新缓冲（越界 clamp，start>=end 空）
let f = v.first();               // Option<T>：空容器 → None，值拷贝
let l = v.last();                // Option<T>
v.reverse();                     // 原地反转
v.swap(0, 1);                    // 交换下标元素
let idx = v.binary_search(&30);  // 返回 index（要求已排序）
```

⚠️ 规划（未实现）：`pop` / `get` / `iter` / `sort` / `with_capacity` / `is_empty`。

## HashMap<K, V>（✅ 基本）

```zeta
let m: HashMap<String, i64> = HashMap::new();
m.insert(String::from("a"), 1);
let v = m.get(String::from("a"));   // 返回 Option<V>（不存在 → None）
```

⚠️ 规划（未实现）：`remove` / `contains_key` / `iter` / `with_capacity`。

## Option<T> / Result<T, E>（✅）

```zeta
let o = Some(42);
o.unwrap_or(0);                  // 42
let r = Result::Err("boom");     // 裸 Err 可自动定型（Infer）
r.unwrap_or(100);                // 100
match o { Some(x) => x, None => 0 }          // 解构（嵌套泛型也可用）
match r { Ok(v) => v, Err(e) => -1 }
```

⚠️ 规划（未实现）：`unwrap` / `map` / `and_then` / `?` 运算符。

## 时间（✅ `time` 子模块）

```zeta
let d = Duration { micros: 90000000 };   // 字面量构造
d.secs();        // 90（micros→秒 整除）
d.millis();      // 90000
d.micros();      // 90000000
d.nanos();       // 90000000000（乘法）
let t = Instant::now();
let el = t.elapsed();            // Duration，CPU 时钟差，恒 >= 0
```

⚠️ 规划形态（未实现）：`Duration::seconds(n)` / `as_secs` / `duration_since`。

## IO（✅ `io` 子模块，自由函数）

```zeta
let s = io::read_file(String::from("a.txt"));   // 读取文件全部内容 → String
io::write_file(String::from("a.txt"), s);       // 写入文件
```

控制台输出用内建 `print` / `println`。⚠️ 规划（未实现）：`File` 结构体、`stdin`/`stdout` 模块、NIO、sendfile。

## 网络（✅ `net` 子模块，基础）

socket 绑定、字节序（网络字节序工具）、主机名查询。⚠️ 规划（未实现）：TCP/UDP/HTTP 封装。

## 同步（✅ `sync` 子模块）

`Mutex` / `RwLock`（底层 pthread 锁）。⚠️ 规划（未实现）：`Condvar` / `Channel`。

## 规划模块（MVP 全部未实现）

`Iterator` trait / `collect`、`fmt`（Display/Debug/format）、`serde`（json/toml）、`async` 运行时、
`Rc`/`Arc`/`Gc` 智能指针、`?` 错误传播与 `IoError` 类型体系。
