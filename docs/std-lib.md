# Zeta 标准库 API 规范

> 版本：v2.0  
> 最后更新：2026-08-22

> **⚠️ 实现状态**：本文为**目标标准库规范**（含规划中 API）。MVP 已实现部分位于
> `crates/zeta-std/zeta/`（**目录化模块**：`core.zeta` 根模块 + `time/`、`sync/`、`io/`、
> `net/`、`fs/` 子目录（`<name>/mod.zeta` + 类型独立文件），driver 加载时经模块展开 +
> use 重新导出合入，用户侧裸名即用）。
> 根模块保留 String / Vec / HashMap / Option / Result 等编译器特判类型；未实现章节属规划（详见下方总览）。
> 教程与可运行示例见 [`guide.md`](./guide.md)；实际 API 以各模块源码为准。

## MVP 实现状态总览

> **规划阶段标注**：剩余任务（📋/🔧）的开发计划见 [`mvp-gaps-plan.md`](./mvp-gaps-plan.md) §3b（阶段 M–T，表格末列标注归属阶段）。

| 章节 | 状态 | MVP 实际形态 | 规划阶段 |
|------|------|--------------|---------|
| §2.1 Option / §2.2 Result | ✅ 已实现 | 泛型 enum + `is_some/is_none/unwrap/unwrap_or/expect` 等 | — |
| §2.3 Iterator | 📋 规划 | `for i in 0..<10` 数值区间可用，trait 未实现 | T2 |
| §3.1 Vec / §3.2 HashMap / §3.3 String | ✅ 已实现（核心 API） | 含 `Vec::slice` 动态切片、`String::from`（字面量/变量）；目标 API 未全部补齐 | T1（目标 API 补齐） |
| §4.1 File | 🔧 部分（自由函数已实现） | `read_file/write_file/append_file`（libc stdio 封装，M3a 起 Result 化：`Result<T, IoError>`） | N1（前置 M，M3a 已完成） |
| §4.2 标准输入输出 | ✅ 已实现 | `stdout`/`stderr` 模块（`write`/`writeln`/`flush`）+ stdin `read_to_string`/`lines` + `eprintln!`/`eprint!` 宏（N4 ✅） | — |
| §4.3 路径与文件系统 | 📋 规划 | — | N3 |
| §4.4 NIO | ✅ 已实现 | `Interest`/`Event`/`Poller` + `set_nonblocking`/`is_nonblocking`（R1a/R1b/R2 ✅，`io/nio.zeta` 基于 poll(2) 封装 + fcntl O_NONBLOCK，`Result<T, IoError>`） | — |
| §4.5 sendfile | ✅ 已实现 | `sendfile` 自由函数 + `File::sendfile_to`（R3 ✅，driver 注入平台内建 `__zeta_sendfile`，macOS sendfile(2) 6 参签名零拷贝） | — |
| §5.1 TCP | ✅ 已实现 | `SocketAddr`/`TcpListener`/`TcpStream` + `read/write/read_line/shutdown`（O1/O2 ✅，libc extern FFI，`Result<T, IoError>`）；旧自由函数保留兼容 | — |
| §5.2 HTTP | ✅ 已实现（同步 MVP） | `HttpClient::get/post` + `Response::status/text` + `json::parse::<T>` 反序列化（O3 ✅，`Connection: close` 无复用）；async 版规划 | S3（async） |
| §6.1 Mutex | ✅ 已实现 | `Mutex`/`RwLock` 裸 `lock/unlock/try_*` + `lock_guard()` guard 语义（作用域尾自动解锁注入）；`Condvar::wait/notify_one/notify_all` + `Barrier`（P1–P3 ✅，`sync/mod.zeta` pthread extern FFI） | — |
| §6.2 Channel | ✅ 已实现 | `channel()` → `ChannelPair { tx, rx }` + `Sender::send/try_send` + `Receiver::recv/try_recv/close/iter`（P1 ✅，`Rc<Channel>` 共享；MVP 非泛型、元素 `i64`、无界） | S3（async） |
| §7 时间 | ✅ 已实现 | `Duration`/`Instant`（libc `clock()` extern） | — |
| §8 格式化与打印 | 🔧 部分 | **内置格式化宏已实现**（I2：`println!`/`print!`/`format!`/`dbg!` + N4 `eprintln!`/`eprint!`（stderr），`{}`/`{:?}` 占位）；**`Display`/`Debug` trait + `Formatter` 已实现**（Q3 ✅，`fmt/mod.zeta`，`{}` 查 `Display::fmt`、`{:?}` 查 `Debug::fmt_debug`） | Q4 |
| §9 序列化 | 🔧 部分 | **`json::stringify`/`json::parse::<T>` 编译器内建已实现**（L2 ✅，含 HashMap + struct 反序列化）；`Serialize` trait + `#[derive(Serialize, Deserialize)]` 标记 + 手写 impl 已实现（Q1 ✅，`serde/mod.zeta`）；**泛型 API 入口 `to_string`/`from_str` + 流式 `to_writer`/`from_reader` 已实现**（Q2 ✅，typecheck 内建别名/desugar）；`Deserialize` trait（`-> Self` 未支持）与 TOML 规划 | Q4 |
| §10 异步运行时 | 🔧 部分 | 普通函数 `async fn`/`.await` 已支持（L1 ✅，MVP 同步语义）；actor 的 `async` 方法 + `.await`/`send` 已实现（独立机制）；`Future`/executor 规划 | S（S0 线程 → S1–S3） |
| §11 智能指针 | 🔧 部分 | `Box<T>`（K2）/ `Rc<T>`/`Arc<T>`/`Weak<T>`（K3）编译器内建已实现；`Gc<T>`（K4）✅ 已实现（MVP，见 §11）；目标 API 未全部补齐 | T3 |
| §12 错误处理 | 🔧 部分 | `Option`/`Result` + `expect/unwrap_or` 已实现；**`?` 运算符已实现**（K1）；`Error`/`From`/`Into` trait 与 `IoError` 定义规划 | M |

> 状态标记：✅ 已实现　🔧 部分实现（注明差异）　📋 规划中（目标 API，MVP 未实现）

---

## 相关文档

| 类型 | 文档 | 说明 |
|------|------|------|
| 项目总纲 | [CODEBUDDY.md](../CODEBUDDY.md) | 项目全景 |
| 设计文档 | [10_标准库规划](../design/10_标准库规划.md) | 模块规划（目标架构） |
| 实现任务 | [P009](../prompts/P009_标准库核心模块.md) | 标准库实现 |
| 剩余任务计划 | [mvp-gaps-plan.md](./mvp-gaps-plan.md) §3b | 阶段 M–T 开发计划（本表📋/🔧章节的规划归属） |

---

## 1. 标准库架构

> **实现状态（2026-08-23）**：已完成**目录化模块拆分**（对齐下述目标架构的目录 + 类型独立文件
> 形式；`pub use` 以根模块 `use` 重新导出等价实现）。实际布局为
> `zeta-std/zeta/core.zeta`（根模块：String / Vec / HashMap / Option / Result + 全部 extern 声明 +
> `mod` 声明 + use 重新导出，指向各类型文件完整路径）+ 子目录：
> `time/`（Duration / Instant）、`sync/`（pthread 锁）、`io/`（mod.zeta 聚合 OpenMode/c_str +
> error.zeta 错误类型 + file.zeta 文件对象 + console.zeta 控制台）、`net/`（mod.zeta 聚合 socket
> 自由函数 + byteorder.zeta 字节打包 + addr.zeta 地址 + tcp.zeta 流与监听 + http.zeta HTTP 客户端）、
> `fs/`（mod.zeta 聚合 fs 函数 + path.zeta 路径对象）。
> **符号完整路径随拆分变更**：如 `io::IoError` → `io::error::IoError`、`net::SocketAddr` →
> `net::addr::SocketAddr`、`net::TcpStream` → `net::tcp::TcpStream`、`fs::Path` → `fs::path::Path`；
> 用户侧裸名 API 不变（core.zeta use 重新导出）。下述**目标架构**（规划：按 crate 目录 + 各类型
> 独立文件 + `pub use` 重导出）中 alloc/collections/fmt/serde/async 等目录为规划内容。

```
zeta-std/
├── core/           ← 最核心的类型和 trait（无依赖）
│   ├── mod.zeta
│   ├── option.zeta
│   ├── result.zeta
│   ├── iter.zeta
│   └── ops.zeta
│
├── alloc/          ← 堆分配原语
│   ├── box.zeta
│   ├── rc.zeta
│   └── arc.zeta
│
├── collections/    ← 集合类型
│   ├── vec.zeta
│   ├── hashmap.zeta
│   ├── hashset.zeta
│   ├── btree.zeta
│   └── deque.zeta
│
├── io/            ← 输入输出
│   ├── file.zeta
│   ├── stdin.zeta
│   ├── stdout.zeta
│   ├── error.zeta
│   ├── nio.zeta     ← 非阻塞 IO（Interest / Event / Poller）
│   └── sendfile.zeta ← 内核零拷贝文件传输
│
├── net/           ← 网络
│   ├── tcp.zeta
│   ├── udp.zeta
│   └── http.zeta
│
├── sync/          ← 同步原语
│   ├── mutex.zeta
│   ├── rwlock.zeta
│   ├── condvar.zeta
│   └── channel.zeta
│
├── time/          ← 时间
│   ├── instant.zeta
│   ├── duration.zeta
│   └── system.zeta
│
├── fmt/           ← 格式化
│   ├── display.zeta
│   ├── debug.zeta
│   └── format.zeta
│
├── serde/         ← 序列化框架
│   ├── mod.zeta
│   ├── json.zeta
│   └── toml.zeta
│
└── async/         ← 异步运行时
    ├── future.zeta
    ├── executor.zeta
    └── task.zeta
```

---

## 2. 核心类型

### 2.1 Option<T>

```zeta
enum Option<T> {
    Some(T),
    None,
}

impl<T> Option<T> {
    /// 返回 Some(value)，如果 value 不为 null
    fn from(value: T) -> Option<T>;
    
    /// 解包，若为 None 则 panic
    fn unwrap(self) -> T;
    
    /// 解包，若为 None 则返回默认值
    fn unwrap_or(self, default: T) -> T;
    
    /// 映射内部值
    fn map<U>(self, f: Fn(T) -> U) -> Option<U>;
    
    /// 扁平映射
    fn and_then<U>(self, f: Fn(T) -> Option<U>) -> Option<U>;
    
    /// 是否为 Some
    fn is_some(&self) -> bool;
    
    /// 是否为 None
    fn is_none(&self) -> bool;
}
```

### 2.2 Result<T, E>

```zeta
enum Result<T, E> {
    Ok(T),
    Err(E),
}

impl<T, E> Result<T, E> {
    /// 解包 Ok 值，若为 Err 则 panic
    fn unwrap(self) -> T;
    
    /// 解包 Ok 值，若为 Err 则返回默认值
    fn unwrap_or(self, default: T) -> T;
    
    /// 映射 Ok 值
    fn map<U>(self, f: Fn(T) -> U) -> Result<U, E>;
    
    /// 映射 Err 值
    fn map_err<F>(self, f: Fn(E) -> F) -> Result<T, F>;
    
    /// 扁平映射
    fn and_then<U>(self, f: Fn(T) -> Result<U, E>) -> Result<U, E>;
    
    /// 错误传播
    fn propagate(self) -> Result<T, E>;  // 等价于 ? 运算符
}
```

### 2.3 Iterator

```zeta
trait Iterator {
    type Item;
    
    fn next(&mut self) -> Option<Self::Item>;
    
    // 默认方法
    fn map<B>(self, f: Fn(Self::Item) -> B) -> Map<Self, B>;
    fn filter(self, pred: Fn(&Self::Item) -> bool) -> Filter<Self>;
    fn collect<B: FromIterator<Self::Item>>(self) -> B;
    fn fold<B>(self, init: B, f: Fn(B, Self::Item) -> B) -> B;
    fn count(self) -> usize;
    fn sum(self) -> Self::Item where Self::Item: Add;
    fn take(self, n: usize) -> Take<Self>;
    fn skip(self, n: usize) -> Skip<Self>;
    fn chain<U: Iterator>(self, other: U) -> Chain<Self, U>;
    fn enumerate(self) -> Enumerate<Self>;
    fn find(self, pred: Fn(&Self::Item) -> bool) -> Option<Self::Item>;
    fn any(self, pred: Fn(&Self::Item) -> bool) -> bool;
    fn all(self, pred: Fn(&Self::Item) -> bool) -> bool;
}
```

---

## 3. 集合类型

### 3.1 Vec<T>

```zeta
struct Vec<T> {
    ptr: *mut T,
    len: usize,
    cap: usize,
}

impl<T> Vec<T> {
    /// 创建空 Vec
    fn new() -> Vec<T>;
    
    /// 创建并预分配容量
    fn with_capacity(cap: usize) -> Vec<T>;
    
    /// 在区域中创建
    fn new_in(region: &Region) -> Vec<T>;
    
    /// 追加元素
    fn push(&mut self, value: T);
    
    /// 弹出元素
    fn pop(&mut self) -> Option<T>;
    
    /// 按索引访问
    fn get(&self, index: usize) -> Option<&T>;
    fn get_mut(&mut self, index: usize) -> Option<&mut T>;
    
    /// 长度
    fn len(&self) -> usize;
    
    /// 是否为空
    fn is_empty(&self) -> bool;
    
    /// 迭代器
    fn iter(&self) -> Iter<'_, T>;
    fn iter_mut(&mut self) -> IterMut<'_, T>;
    
    /// 排序
    fn sort(&mut self) where T: Ord;
    fn sort_by(&mut self, cmp: Fn(&T, &T) -> Ordering);
    
    /// 二分查找
    fn binary_search(&self, value: &T) -> Result<usize, usize> where T: Ord;
}
```

### 3.2 HashMap<K, V>

```zeta
struct HashMap<K, V> {
    // 内部哈希表
}

impl<K, V> HashMap<K, V> where K: Hash + Eq {
    fn new() -> HashMap<K, V>;
    fn with_capacity(cap: usize) -> HashMap<K, V>;
    fn insert(&mut self, key: K, value: V) -> Option<V>;
    fn get(&self, key: &K) -> Option<&V>;
    fn get_mut(&mut self, key: &K) -> Option<&mut V>;
    fn remove(&mut self, key: &K) -> Option<V>;
    fn contains_key(&self, key: &K) -> bool;
    fn len(&self) -> usize;
    fn iter(&self) -> Iter<'_, K, V>;
}
```

### 3.3 String

```zeta
struct String {
    vec: Vec<u8>,  // UTF-8 编码
}

impl String {
    fn new() -> String;
    fn from(s: &str) -> String;
    fn push(&mut self, ch: char);
    fn push_str(&mut self, s: &str);
    fn len(&self) -> usize;       // 字节长度
    fn chars(&self) -> Chars;
    fn lines(&self) -> Lines;
    fn split(&self, sep: &str) -> Split;
    fn replace(&self, from: &str, to: &str) -> String;
    fn to_uppercase(&self) -> String;
    fn to_lowercase(&self) -> String;
    fn trim(&self) -> &str;
}
```

---

## 4. IO 模块

### 4.1 File

```zeta
struct File {
    fd: i32,
    path: String,
    mode: OpenMode,
}

enum OpenMode {
    Read,
    Write,
    Append,
    ReadWrite,
}

impl File {
    /// 打开文件
    fn open(path: &str) -> Result<File, IoError>;
    
    /// 创建文件
    fn create(path: &str) -> Result<File, IoError>;
    
    /// 打开（自定义模式）
    fn open_with(path: &str, mode: OpenMode) -> Result<File, IoError>;
    
    /// 读取所有内容
    fn read_to_string(&mut self) -> Result<String, IoError>;
    
    /// 读取到缓冲区
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError>;
    
    /// 写入
    fn write(&mut self, buf: &[u8]) -> Result<usize, IoError>;
    fn write_all(&mut self, buf: &[u8]) -> Result<(), IoError>;
    
    /// 刷新
    fn flush(&mut self) -> Result<(), IoError>;
    
    /// 文件大小
    fn metadata(&self) -> Result<Metadata, IoError>;
}
```

### 4.2 标准输入输出

```zeta
mod stdin {
    fn read_line() -> Result<String, IoError>;
    fn read_to_string() -> Result<String, IoError>;
    fn lines() -> Lines;
}

mod stdout {
    fn write(s: &str) -> Result<(), IoError>;
    fn writeln(s: &str) -> Result<(), IoError>;
    fn flush() -> Result<(), IoError>;
}

mod stderr {
    fn write(s: &str) -> Result<(), IoError>;
    fn writeln(s: &str) -> Result<(), IoError>;
}
```

### 4.3 路径与文件系统

```zeta
struct Path {
    components: Vec<String>,
}

impl Path {
    fn new(s: &str) -> Path;
    fn join(&self, other: &str) -> Path;
    fn parent(&self) -> Option<Path>;
    fn file_name(&self) -> Option<&str>;
    fn extension(&self) -> Option<&str>;
    fn exists(&self) -> bool;
    fn is_file(&self) -> bool;
    fn is_dir(&self) -> bool;
}

// 文件系统操作
mod fs {
    fn read_to_string(path: &str) -> Result<String, IoError>;
    fn write(path: &str, contents: &str) -> Result<(), IoError>;
    fn copy(from: &str, to: &str) -> Result<usize, IoError>;
    fn remove(path: &str) -> Result<(), IoError>;
    fn rename(from: &str, to: &str) -> Result<(), IoError>;
    fn create_dir(path: &str) -> Result<(), IoError>;
    fn create_dir_all(path: &str) -> Result<(), IoError>;
    fn read_dir(path: &str) -> Result<ReadDir, IoError>;
}
```

### 4.4 非阻塞 IO（NIO）

NIO 提供非阻塞模式设置与跨平台事件轮询，是构建事件驱动服务器的基础。
MVP 基于 `poll(2)` 实现（POSIX 统一，macOS/Linux/FreeBSD 均可用），
`register`/`deregister`/`reregister`/`poll` + `POLLIN`/`POLLOUT` 映射；
规划中「Linux `epoll` / macOS `kqueue`」为高性能替代。
非阻塞模式经 `fcntl(F_GETFL/F_SETFL)` 设置 `O_NONBLOCK`（macOS/Linux 均为 0x4）。
WASI 下无 `poll`/`fcntl` 语义，相关函数短路返回 `Err`（禁用文档化，L4a）。

```zeta
// 事件关注标志
enum Interest {
    Readable,          // 只关注可读
    Writable,          // 只关注可写
    ReadableWritable,  // 同时关注可读与可写
}

// 就绪事件：token 用于在应用中定位对应的 fd；interest 为实际就绪方向
// （由 poll 返回的 revents 解析；POLLERR/POLLHUP 无关注位时按可读报告）
struct Event {
    token: i64,
    interest: Interest,
}

impl Event {
    fn is_readable(&self) -> bool;
    fn is_writable(&self) -> bool;
}

// 事件轮询器（poll(2) 封装；MVP 无平台句柄，注册表状态直接暴露）
struct Poller {
    fds: Vec<i64>,      // 已注册 fd
    events: Vec<i64>,   // 关注掩码（POLLIN=1 / POLLOUT=4）
    tokens: Vec<i64>,   // 应用侧 token（与 fds 对齐）
}

impl Poller {
    /// 创建轮询器（poll(2) 无显式创建，Poller 仅为注册表状态容器）
    fn new() -> Result<Poller, IoError>;

    /// 注册 fd，绑定应用侧 token；重复注册报 AlreadyExists
    fn register(&mut self, fd: i64, token: i64, interest: Interest) -> Result<i64, IoError>;

    /// 修改 fd 的关注事件与 token；未注册则追加注册（与 register 等价）
    fn reregister(&mut self, fd: i64, token: i64, interest: Interest) -> Result<i64, IoError>;

    /// 注销 fd；未注册报 NotFound
    fn deregister(&mut self, fd: i64) -> Result<i64, IoError>;

    /// 阻塞等待就绪事件；timeout_ms < 0 表示无限等待，
    /// 返回本次就绪的事件列表（每次调用重新扫描 revents）
    fn poll(&self, timeout_ms: i64) -> Result<Vec<Event>, IoError>;
}

// 设置 fd 是否为非阻塞模式（fcntl F_GETFL=3 / F_SETFL=4；O_NONBLOCK=0x4）
fn set_nonblocking(fd: i64, nonblocking: bool) -> Result<i64, IoError>;

// 查询 fd 是否处于非阻塞模式
fn is_nonblocking(fd: i64) -> Result<bool, IoError>;
```

典型的事件循环：

```zeta
set_nonblocking(listener.fd, true)?;
let mut poller = Poller::new()?;
poller.register(listener.fd, 0, Interest::Readable)?;
loop {
    let events = poller.poll(100)?;   // 100ms 超时；-1 表示无限等待
    for e in events {
        match e.token {
            0 => accept_connection(listener)?,
            t => handle_conn(t, e.interest)?,
        }
    }
}
```

### 4.5 零拷贝传输（sendfile）

`sendfile` 在内核态将文件内容直接拷贝到 socket，不经过用户态缓冲区，
适用于静态文件响应、大文件传输代理等场景。

```zeta
mod sendfile {
    /// 将 in_fd 从 offset 处开始的内容零拷贝发送到 out_fd；
    /// count == 0 表示发送到文件末尾（EOF）
    fn sendfile(out_fd: i32, in_fd: i32, offset: u64, count: usize) -> Result<usize, IoError>;
}

impl File {
    /// 便捷方法：将整个文件（offset 起至 EOF）发送到 socket
    fn sendfile_to(&self, sock_fd: i32, offset: u64) -> Result<u64, IoError>;
}

// 示例：HTTP 静态文件响应（无需把文件读进内存）
let n = sendfile(conn.fd, file.fd, 0, 0)?;   // 发送整个文件
let n = sendfile(conn.fd, file.fd, 64, 4096)?; // 只发送中间一段
```

> 约束：`in_fd` 须为**可读 fd**（O_RDONLY），`File::create` 的 O_WRONLY 句柄直接 sendfile 会返回 `EBADF`——
> 正确用法为先 `write`+`flush`+`close` 落盘，再 `File::open(path, io::OpenMode::Read)` 以只读模式重新打开取 fd。
>
> 平台差异：Linux 与 macOS/BSD 的 `sendfile(2)` 参数顺序不同，由绑定层屏蔽；
> 非 Unix 平台（Windows/WASM）当前返回 `Unsupported`，Windows 计划用 `TransmitFile` 补齐。

---

## 5. 网络模块

> O 阶段（2026-08）✅：TCP 对象化 + HTTP 同步 MVP 已实现。实现方式为 `net/` 子目录（`net/mod.zeta` 聚合 socket 自由函数 + `net/byteorder.zeta` 字节打包 + `net/addr.zeta` 地址 + `net/tcp.zeta` 流与监听 + `net/http.zeta` HTTP 客户端）直接 libc extern FFI（无 Rust 绑定层），`socklen_t` 以 8 字节小端缓冲传递；平台自适应 sockaddr_in 布局（macOS `sin_len` 头 vs Linux `sin_family`，`__zeta_target_os()` 区分）；WASI（码 5）网络函数短路返回 `Err`。符号完整路径见各文件（如 `net::addr::SocketAddr`）；示例见 `tests/run-pass/tcp_addr.zeta`、`tcp_echo.zeta` 与 `crates/zeta-driver/tests/net_http_test.rs`。

### 5.1 TCP（✅ 已实现）

```zeta
struct Ipv4Octets { a: i64, b: i64, c: i64, d: i64 }
fn ipv4_octets(ip: String) -> Result<Ipv4Octets, IoError>;   // "127.0.0.1" → 4 段解析（O1a）

struct SocketAddr {
    ip: String,          // "a.b.c.d"
    port: i64,
}

impl SocketAddr {
    fn new(ip: String, port: i64) -> SocketAddr;   // 字符串 IP 打包
    fn ip(&self) -> String;                        // 原样返回 IP 字符串
    fn port(&self) -> i64;
    fn parse(s: String) -> Result<SocketAddr, IoError>;  // "host:port"（最后一个 ':' 分隔，IPv4 校验）
    fn to_sockaddr(&self) -> String;               // 平台 sockaddr_in 打包（macOS sin_len 头 vs Linux，socklen_t 小端缓冲）
}

enum Shutdown { Read, Write, Both }

struct TcpListener {
    fd: i64,
    addr: SocketAddr,
}

impl TcpListener {
    fn bind(addr: SocketAddr) -> Result<TcpListener, IoError>;  // SO_REUSEADDR
    fn accept(&self) -> Result<TcpStream, IoError>;
    fn local_addr(&self) -> Result<SocketAddr, IoError>;
}

struct TcpStream {
    fd: i64,
    addr: SocketAddr,
}

impl TcpStream {
    fn connect(addr: SocketAddr) -> Result<TcpStream, IoError>;
    fn read(&self, cap: i64) -> Result<String, IoError>;        // 读至多 cap 字节（recv_some 包装）
    fn write(&self, buf: String) -> Result<i64, IoError>;       // 全量发送（send_all 包装）
    fn peer_addr(&self) -> SocketAddr;
    fn read_line(&self) -> Result<String, IoError>;             // O2 逐行读取 helper（读到 '\n'）
    fn shutdown(&self, how: Shutdown) -> Result<i64, IoError>;
}
```

> 旧自由函数 `tcp_connect`/`socketpair_stream`/`send_all`/`recv_some`/`hostname` 保留兼容；`tcp_connect` 现返回 `TcpStream`（自由函数兼容壳）。O2 字节读写以 String 缓冲承载（`read(cap)` 按容量读、`write(String)` 全量发），`read_line` 提供逐行语义。

### 5.2 HTTP 同步 MVP（✅ 已实现，O3）

```zeta
struct ParsedUrl { host: String, port: i64, path: String }
fn parse_url(url: String) -> Result<ParsedUrl, IoError>;   // http://host[:port]/path 拆分（默认端口 80、路径 "/"）

struct HttpClient {
    _unit: i64,                                 // 占位字段（规避空结构体构造限制）
}

impl HttpClient {
    fn new() -> HttpClient;
    fn get(url: String) -> Result<Response, IoError>;
    fn post(url: String, body: String) -> Result<Response, IoError>;
}

struct Response {
    status: i64,
    body: String,
}

impl Response {
    fn status(&self) -> i64;                    // 200 / 404 / ...
    fn text(&self) -> String;                   // body 原文
}
```

> MVP 语义：每次请求新建连接 + `Connection: close`（无连接复用）；`json::parse::<T>` 直接对 `r.text()` 反序列化（返回裸 `T` 非 `Result`）。async 版 `get_async`/`post_async` 随阶段 S3 规划。

---

## 6. 同步原语

### 6.1 Mutex

```zeta
struct Mutex<T> {
    inner: RawMutex,
    data: UnsafeCell<T>,
}

impl<T> Mutex<T> {
    fn new(value: T) -> Mutex<T>;
    fn lock(&self) -> MutexGuard<T>;
    fn try_lock(&self) -> Option<MutexGuard<T>>;
}

struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
}

impl<T> Deref for MutexGuard<'_, T> {
    fn deref(&self) -> &T;
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T;
}
// Guard 离开作用域时自动解锁
```

### 6.2 Channel

```zeta
struct Sender<T> {
    queue: Arc<LockFreeQueue<T>>,
}

struct Receiver<T> {
    queue: Arc<LockFreeQueue<T>>,
}

/// 创建无界 channel
fn channel<T>() -> (Sender<T>, Receiver<T>);

/// 创建有界 channel
fn bounded_channel<T>(capacity: usize) -> (Sender<T>, Receiver<T>);

impl<T> Sender<T> {
    fn send(&self, value: T) -> Result<(), SendError<T>>;
    fn try_send(&self, value: T) -> Result<(), TrySendError<T>>;
}

impl<T> Receiver<T> {
    fn recv(&self) -> Result<T, RecvError>;
    fn try_recv(&self) -> Result<T, TryRecvError>;
    async fn recv_async(&self) -> Result<T, RecvError>;
}
```

> **MVP 已实现（P1 ✅，`sync/mod.zeta`）**：目标 API 为泛型 + `Arc` 无锁队列；MVP 为非泛型 `i64` 元素 + `Rc<Channel>` 共享（Mutex + Condvar 队列），构造为 `let p = channel(); p.tx / p.rx`（`ChannelPair` 结构体含 `tx`/`rx` 槽，未提供多元组返回）。差异：`try_send` 无界队列恒 `true`；`recv`/`try_recv` 返回 `Option<i64>`（空/close 后 `None`）；`recv_async` 规划（S3 async）。

---

## 7. 时间模块

```zeta
struct Instant {
    nanos: u64,
}

struct Duration {
    nanos: u64,
}

impl Duration {
    fn seconds(n: u64) -> Duration;
    fn milliseconds(n: u64) -> Duration;
    fn microseconds(n: u64) -> Duration;
    fn nanoseconds(n: u64) -> Duration;
    fn from_secs_f64(secs: f64) -> Duration;
    fn as_secs(&self) -> u64;
    fn as_millis(&self) -> u128;
    fn as_nanos(&self) -> u128;
}
// 实现现状（2026-08-25）：存储 micros: i64（微秒基准）；构造器 `seconds(n: i64)`/
// `milliseconds(n: i64)` 已实现（S2a 依赖）；读方法 `secs`/`millis`/`micros`/`nanos`
// （self，下取整）。`Instant::now`/`elapsed` 基于**墙钟**（`__zeta_clock_monotonic`
// = clock_gettime CLOCK_MONOTONIC，S2b ✅，睡眠期间推进；不支持平台返回 -1 时
// 退回 clock() CPU 时钟，恒可用）；`nanos: u64` 布局 / `from_secs_f64` / `as_*` 规划中。

impl Instant {
    fn now() -> Instant;
    fn duration_since(&self, earlier: Instant) -> Duration;
    fn elapsed(&self) -> Duration;
}
```

---

## 8. 格式化与打印

> **MVP 现状**：内置格式化宏已实现（I2 + N4）：`println!`/`print!`/`format!`/`dbg!`（stdout）+ `eprintln!`/`eprint!`（stderr，N4），支持 `{}`/`{:?}` 值占位、`{{`/`}}` 转义、多参数可变长度；desugar 为 String 拼接 + 内建打印。内建函数 `println(expr)` / `print(expr)` / `eprintln(expr)` / `eprint(expr)` 亦可用（0–1 参数，自动按类型输出）。`Display`/`Debug` trait 为规划 API。

```zeta
println("Hello, Zeta!");     // 字符串字面量
println(42);                 // 整型
println(3.14);               // 浮点
println(true);               // bool
println();                   // 空行
print(x);                    // 不换行
```

以下为目标 API（规划）：

```zeta
/// 格式化 trait（规划）
trait Display {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>;
}

trait Debug {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>;
}

/// 打印宏（规划）
println!("Hello, {}!", name);       // 输出 + 换行
print!("Progress: {}%", pct);       // 输出不换行
eprintln!("Error: {}", msg);        // 输出到 stderr
dbg!(value);                        // 调试输出（带位置信息）

/// 格式化字符串（规划）
let s = format!("{} + {} = {}", a, b, a + b);
```
> **已实现（Q3 ✅，2026-08，§8 格式化引擎接入）**：`println!`/`print!`/`format!`/`dbg!` 宏
> （I2）+ N4 `eprintln!`/`eprint!`（stderr）已实现；**`Display`/`Debug` trait + `Formatter` 已实现**
> （std `fmt/mod.zeta`，`core.zeta` `mod fmt;` + `use fmt::{Display, Debug, Formatter}`）。
> MVP 签名降级（与上述目标 API 差异）：
> - `trait Display { fn fmt(&self, f: &mut Formatter) -> String; }`——fmt 直接返回显示字符串
>   （String 拼接模式，与 `serde::Serialize::to_json` 同构；目标 `Result<(), FmtError>` 未支持）。
> - `trait Debug { fn fmt_debug(&self, f: &mut Formatter) -> String; }`——方法名 `fmt_debug`
>   避免与 `Display::fmt` 同名冲突（MVP 方法调用按名查找 impl，inherent 优先、trait 次之）。
> - `struct Formatter { buf: String }` + `Formatter::new()`——约定占位类型（引擎生成
>   `{ let mut __fmt_q3 = Formatter::new(); x.fmt(&mut __fmt_q3) }` 传入，fmt 体可不使用；
>   `&mut` 仅支持变量目标，临时值不可用）。
> - `{}` 占位符：内建类型（i64/bool/String/&str/字符串字面量）走内建转换；自定义类型查
>   `Display` impl（存在 `fmt` 方法）走 `x.fmt(&mut Formatter::new())`；无 impl 报 Unsupported
>   提示 `impl Display`。`{:?}` 同构走 `Debug`（`fmt_debug`）；`dbg!` 用 Debug 格式。
> 配套修复：模块内 trait/impl 方法签名收集阶段即 `resolve_ast_type`，use 段尚未注册——
> `resolve_full_name` 加当前模块前缀回退 + `collect_item_decls`/`collect_mod_types_inner`/
> `check_item` 的 ModDecl 分支设置 `module_prefix`（模块内短名按 `mod::Name` 定位）。

---

## 9. 序列化框架

> **实现状态（2026-08-25）**：🔧 部分（Q1–Q2 ✅）。`json` 模块的 `stringify` / `parse` 已实现（L2 ✅，编译器内建 desugar，见 §9.1）；**`Serialize` trait + `#[derive(Serialize, Deserialize)]` 标记 + struct 反序列化已实现（Q1 ✅，2026-08，§9.1b）**：`serde/mod.zeta` 定义 `trait Serialize { fn to_json(&self) -> String; }`（自定义类型可手写 impl 并经 `to_json()` 调用，内建类型默认 impl 为声明性——MVP 内建类型方法调用不走 trait impl 查找，序列化经 `json::stringify` 特判）；`#[derive(...)]` 语法经 lexer `Pound` + parser 特判解析（`AstStructDecl.derive`）；`json::parse::<T>` 支持 struct（字段名匹配、顺序无关、缺失字段零值、未知字段忽略、嵌套 struct）。**泛型 API 入口 + 流式 writer/reader 已实现（Q2 ✅，2026-08，§9.2）**：`json::to_string(v)` ≡ `json::stringify(v)`、`json::from_str::<T>(s)` ≡ `json::parse::<T>(s)`（typecheck 内建别名；`T: Serialize`/`T: Deserialize` trait bound 未支持——MVP 无泛型 trait 约束，签名降级为无 bound turbofish 形式）；`json::to_writer(w, v)` → `w.write_all(json::stringify(v))`（返回 `Result<i64, io::error::IoError>`）、`json::from_reader::<T>(r)` → `json::parse::<T>(r.read_to_string().unwrap())`（读失败经 `unwrap` 死循环 MVP 语义；首参须 `File`/`&File`/`&mut File`，TcpStream 留待流式 read_all 方法化）。`Deserialize` trait（`-> Self` 返回自身类型未支持，见 mvp-gaps-plan.md M2b）与 TOML 模块仍规划。

### 9.1 JSON（L2 ✅，编译器内建）

```zeta
mod json {
    fn stringify(value: T) -> String;   // ✅ 内建：i64 / bool / String / &str / 数组 / struct / Vec / HashMap
    fn parse<T>(s: String) -> T;        // ✅ 内建：i64 / bool / String / HashMap（turbofish 泛型实参 `::<T>`）
}
```

- `json::stringify(v)`：`i64`→十进制；`bool`→`true`/`false`；`String`/`&str`→带引号 JSON 字符串（`"` `\` 换行 制表 转义为 `\"` `\\` `\n` `\t`）；数组→`[e0,e1,...]`（静态展开）；struct→`{"f1":v1,"f2":v2}`（字段序 = 定义序，嵌套递归）；`Vec<T>`→`[e0,e1,...]`（while 循环 push_str）；`HashMap<K,V>`→`{"k":v,...}`（L2f：i64/String 键 + 值递归，键序确定性——按容量扫描 states 顺序）。
- `json::parse::<T>(s)`：turbofish 泛型实参指定目标类型；`i64`→`string_to_int`、`bool`→字节比较、`String`→`json_unescape`（剥离首尾引号 + 还原转义）；`HashMap<K,V>`（L2g：`substring(1, len-1)` 剥离 `{}` → `split(",")` 分段 → `find(":")` 分键值 → 键经 `json_unescape`（i64 键再 `string_to_int`）+ 值递归标量解析 → `HashMap::new()`/`insert` 构建，`let __m: HashMap<K,V>` 注解定型；空 `{}` → 空 map）。**MVP 语义：直接返回 `T`**（非法输入给默认值：`0` / `false` / 空串 / 空 map），非 Result 包装。
- 转义函数 `json_escape` / `json_unescape` 实现于 std `core.zeta`。
- MVP 限制：`map![...]`/`vec![...]` 绑定后 K/V（元素）为 `Infer`，须 `let m: HashMap<i64, i64>`（`let v: Vec<i64>`）注解定型（与 `for x in v` 约束一致）；HashMap parse 键/值含逗号或冒号时 `split(",")`/`find(":")` 分段不可靠、嵌套 `HashMap` 值报 Unsupported（值限标量）；`HashMap<i64,Vec<T>>` 值序列化可用但 parse 不支持；struct parse 的嵌套 struct/Vec/HashMap 字段值含逗号时 `split(",")` 分段不可靠（与 HashMap 分支一致）、泛型 struct 不支持、空 struct 报 Unsupported（详见 §9.1b）。

### 9.1b Serialize trait / derive 标记 / struct 反序列化（Q1 ✅，2026-08）

```zeta
// Q1a：Serialize trait 定义（std serde/mod.zeta；core.zeta 全局 use serde::Serialize）
trait Serialize {
    fn to_json(&self) -> String;
}
// 自定义类型手写 impl（impl 方法查找可用）
struct Wrapped { v: i64 }
impl Serialize for Wrapped {
    fn to_json(&self) -> String { format!("{{\"w\":{}}}", self.v) }
}
let w = Wrapped { v: 7 };
println(w.to_json());            // {"w":7}
// 内建类型（i64/bool/String）默认 impl 为声明性文档：MVP 内建类型方法调用
// 不走 trait impl 查找（`x.to_json()` 报 `i64::to_json not found`），序列化统一
// 走 `json::stringify` 编译器特判。
// `Deserialize` trait 的 `-> Self` 返回自身类型未支持（typecheck undefined type
// `Self`），解析统一经 `json::parse::<T>` 内建（Q1a 预案退化）。

// Q1b：#[derive(Serialize, Deserialize)] 标记（lexer Pound + parser 特判，AST 存储）
#[derive(Serialize, Deserialize)]
struct Point { x: i64, y: i64 }

// Q1c：derive 标记 struct 的序列化/反序列化（round-trip）
let p = Point { x: 1, y: 2 };
let s = json::stringify(p);              // {"x":1,"y":2}（字段序 = 定义序）
let q = json::parse::<Point>(s);         // 反序列化（字段名匹配，顺序任意）
let q2 = json::parse::<Point>("{\"y\":20,\"x\":10}");  // 30：字段顺序无关
let q3 = json::parse::<Point>("{}");     // 缺失字段保持零值（0, 0）
// 嵌套 struct（值不含逗号）+ round-trip 亦可用；未知字段忽略
```

> Q1 语义说明：derive 在 MVP 下为**标记语义**——解析并存储到 `AstStructDecl.derive`，typecheck 不强制检查；序列化/反序列化统一经编译器内建 `json::stringify` / `json::parse::<T>`（struct 分支字段名匹配 desugar，复用 L2 内建字段级能力）。struct 反序列化实现：`String::from` → `substring(1, len-1)` 剥离 `{}` → `split(",")` 分段 → `for` 遍历（`find(":")` 分键值）→ 字段名比较 if-else 链逐字段 `Assign`（值递归 `json_parse_ast`），`let mut __p: T = T { 零值 }` 承载结果。

```zeta
// 目标 API（规划）
trait Serialize {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(), SerError>;
}

trait Deserialize {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self, DeError>;
}

mod json {
    fn to_string<T: Serialize>(value: &T) -> Result<String, JsonError>;
    fn from_str<T: Deserialize>(s: &str) -> Result<T, JsonError>;
    fn to_writer<T: Serialize>(writer: &mut Writer, value: &T) -> Result<(), JsonError>;
    fn from_reader<T: Deserialize>(reader: &mut Reader) -> Result<T, JsonError>;
}
// 已实现（Q2，MVP 签名降级）：
//   json::to_string(v) -> String              // ≡ json::stringify(v)，无 bound、类型由实参推断
//   json::from_str::<T>(s) -> T               // ≡ json::parse::<T>(s)，无 bound、turbofish 指定
//   json::to_writer(w: &mut File, v) -> Result<i64, io::error::IoError>  // w.write_all(json::stringify(v))
//   json::from_reader::<T>(r: &mut File) -> T // json::parse::<T>(r.read_to_string().unwrap())
//   MVP 注：`T: Serialize`/`T: Deserialize` bound 未支持（无泛型 trait 约束）；流式目标
//   限 `File`（TcpStream 留待流式 read_all 方法化）；无 `JsonError`（错误经 IoError/死循环）。
//   std 文件 io/file.zeta + typecheck 内建（check_json_to_writer / check_json_from_reader）。

// TOML
mod toml {
    fn to_string<T: Serialize>(value: &T) -> Result<String, TomlError>;
    fn from_str<T: Deserialize>(s: &str) -> Result<T, TomlError>;
}
```

---

## 10. 异步运行时

> **实现状态（2026-08-25）**：🔧 部分。普通函数 `async fn` / `.await` 已支持（**S1c ✅，2026-08，§10.3 状态机 desugar**：`async fn` 编译为 Future 结构体 + poll 状态机 + 构造器，`expr.await` 经状态机轮询子 future，支持挂起 `Poll::Pending` 与恢复，替代 L1 同步语义）；actor `async` 方法 + `.await` / `send` 为独立机制（§9，ask 同步往返）。**线程支持（S0 ✅，2026-08，§10.1）**：`Thread::start`/`join`/`current` 已实现。**睡眠（S2a ✅，2026-08，§10.1）**：`thread::sleep(Duration)`（usleep 绑定）。**并发收尾（S2b ✅，2026-08，§10.1）**：`join_all`（线程版）+ **墙钟**（clock_gettime MONOTONIC，`Instant::now/elapsed` 睡眠期间推进）。**异步基础（S1a/S1b/S2c ✅，2026-08，§10.2）**：`Poll` 枚举 + `Future` trait + `block_on` 手动轮询 + `timeout` 超时轮询已实现（MVP 退化：Output 固定 i64；timeout 返回 `Result<i64, i64>`）。下方泛型 `join_all`（Future 版）/ `timeout`（Result 版）/ `sync` 并发原语仍为规划 API。

```zeta
// S1a/S1b ✅（2026-08，§10.2）：MVP 退化——关联类型 `type Output` / `Pin` / `Context`
// 规划中（随 S1c async/await 状态机），Output 固定为 i64；`block_on` 为泛型函数
// `block_on<T>(f: &mut T)`（实例化时按具体类型解析 poll，无约束宽松语义）。
trait Future {
    fn poll(&mut self) -> Poll<i64>;
}

enum Poll<T> {
    Ready(T),
    Pending,
}

/// 等待 Future 完成
async fn block_on<F: Future>(future: F) -> F::Output;

/// 并发等待多个 Future
/// MVP 退化（S2b ✅，2026-08）：线程版 `thread::join_all(Vec<Thread>) -> Vec<i64>`
/// （§10.1）——Future 版规划（依赖 S1c async/await 状态机 + 泛型方法）。
async fn join_all<F: Future>(futures: Vec<F>) -> Vec<F::Output>;

/// 超时
/// MVP 退化（S2c ✅，2026-08，§10.2）：`timeout(duration, &mut fut) -> Result<i64, i64>`
/// ——`TimeoutError` 类型规划中，超时统一 `Err(-1)`；Future 版（`F: Future` +
/// `Result<F::Output, TimeoutError>`）规划（依赖 S1c + 泛型约束）。
async fn timeout<F: Future>(duration: Duration, future: F) -> Result<F::Output, TimeoutError>;

/// 并发原语
mod sync {
    fn mutex<T>(value: T) -> AsyncMutex<T>;
    fn rwlock<T>(value: T) -> AsyncRwLock<T>;
    fn notify() -> Notify;
    fn barrier(n: usize) -> Barrier;
}
```

### 10.1 线程（S0，2026-08 ✅；S2a sleep ✅；S2b join_all + 墙钟 ✅）

线程支持为异步运行时前置工程：driver 注入平台内建 `__zeta_thread_spawn` /
`__zeta_thread_join` / `__zeta_thread_self` / `__zeta_thread_sleep`（pthread_create/
join/self + usleep 绑定，与 sendfile 相同架构），语言侧 `thread` 模块
（`crates/zeta-std/zeta/thread/mod.zeta`）。

```zeta
// 线程句柄（pthread_t 的 i64 视图）
struct Thread {
    tid: i64,
}

impl Thread {
    /// 派生新线程运行 f（零参数、返回 i64），立即返回线程句柄；
    /// 启动失败（pthread_create 非零）映射 IoError（M1b）。
    /// 注：`spawn` 为语言保留关键字（actor 派生），方法名取 `start`。
    fn start(f: fn() -> i64) -> Result<Thread, IoError>;

    /// 阻塞等待本线程结束，返回其返回值槽内容（pthread_join）；失败返回 -1。
    fn join(&self) -> i64;

    /// 当前线程的 pthread id（正整数；MVP 无 TLS 需求，仅作标识）。
    fn current() -> i64;
}

/// 阻塞当前线程指定时长（usleep 绑定；micros 截断 u32，上限约 71 分钟）；
/// 返回 0 成功 / -1 失败（WASI/Windows 下 stub 恒 -1，禁用文档化）。
fn sleep(duration: Duration) -> i64;

/// 并发等待一组线程全部完成（S2b，2026-08 ✅），按传入顺序返回各线程
/// 返回值（`join_all(ts)`，ts: `Vec<Thread>`）。线程已由 `Thread::start`
/// 并行派生；join_all 阻塞至全部完成（顺序 join 收尾——join 为阻塞原语，
/// "全部完成才返回"语义与并发收尾等价）。join 失败该位置返回 -1。
/// 注：MVP 退化为线程版；泛型 Future 版（§10 规划 API）依赖 S1c + 泛型方法。
fn join_all(threads: Vec<Thread>) -> Vec<i64>;
```

约束：
- 线程函数须为顶层/模块级 `fn() -> i64`（H1 函数指针，按地址整数经 extern i64 形参传递，codegen `ptrtoint`）；闭包值跨线程捕获规划中（S3）。
- `pthread_join` 返回值槽读取 i64；Zeta 线程函数返回 i64 与 pthread 期望的 `void*` 同寄存器（ABI 安全）。
- `sleep` 经 `usleep(3)`（POSIX 微秒）；`Duration` 构造器 `seconds`/`milliseconds` 见 §8。
- **`Instant::now`/`elapsed` 基于墙钟**（S2b ✅：`__zeta_clock_monotonic` = clock_gettime CLOCK_MONOTONIC，睡眠期间推进——`join_all.zeta` 用例断言 sleep 20ms×3 后 elapsed ≥ 20ms）；不支持平台（freebsd/windows/wasi）返回 -1 时退回 `clock()`（CPU 时钟，CLOCKS_PER_SEC=1e6，睡眠期间不推进）。
- WASI/Windows 下注入 stub 返回 -1，`Thread::start` 返回 `Err`（Unsupported，禁用文档化）。
- MVP 无 TLS / 线程局部状态需求（S0e）。

### 10.2 Future / Poll / block_on / timeout（S1a/S1b/S2c，2026-08 ✅）

异步运行时基础件（`crates/zeta-std/zeta/future.zeta`，core.zeta 重导出 `Future`/`Poll`/`block_on`/`timeout`）。

```zeta
// 轮询结果：Ready(值) / Pending（泛型枚举，与 Option 同构）
enum Poll<T> { Ready(T), Pending }

// Future trait：poll 推进状态机。MVP 退化——关联类型 `type Output` 不支持
// （std-lib.md §10 标注"不可行则退化"），Output 固定为 i64；
// `&mut self` 聚合指针传递（与 M1b 的 nio 一致）。
trait Future { fn poll(&mut self) -> Poll<i64>; }

// 手动轮询：loop { match f.poll() { Ready(v) => return v, Pending => .. } }
// 泛型形态 `block_on<T>(f: &mut T)`——MVP 泛型函数调用点实例化，body 以具体
// 类型重查，`f.poll()` 解析到用户 impl（无 `F: Future` 约束，宽松语义）；
// 调用 `block_on(&mut fut)`，泛型实参由实参类型推断。
fn block_on<T>(f: &mut T) -> i64;

// S2c（2026-08 ✅）：带超时阻塞轮询——`duration` 内未 `Ready` 返回 `Err`。
// MVP 退化：`timeout<T>(duration: Duration, f: &mut T) -> Result<i64, i64>`，
// 超时统一 `Err(-1)`（`TimeoutError` 类型规划中）；参数顺序对齐规划 API
// （duration 在前）；超时判定经墙钟 `__zeta_clock_monotonic`（S2b ✅，与
// `time::Instant` 相同退化逻辑：返回 -1 退回 clock()）——注：MVP 静态方法
// 调用不支持模块路径前缀（`time::Instant::now` 不可用），故直接复用 extern；
// 忙等轮询（与 block_on 一致，事件驱动等待规划随 R1 Poller）。
fn timeout<T>(duration: Duration, f: &mut T) -> Result<i64, i64>;
```

约束：
- Output 固定 i64（关联类型 / `Pin<&mut Self>` / `Context` 规划中）；`block_on`/`timeout` 无 `F: Future` 约束检查（宽松）；dyn 不可作函数参数（H4 MVP 限制），Future 对象须经 `&mut` 传泛型。
- 用户侧：`impl Future for MyFut { fn poll(&mut self) -> Poll<i64> { ... } }` + `block_on(&mut fut)`；三轮轮询示例见 `tests/run-pass/block_on.zeta`（输出 `42` / `3`）；`timeout` 成功/超时示例见 `tests/run-pass/timeout.zeta`（输出 `3` / `-1`）。

### 10.3 async fn 状态机（S1c，2026-08 ✅）

> `async fn` 经编译器 desugar（`crates/zeta-desugar`：`analyze.rs` 段切分 / `generate.rs` 结构体生成）为真实状态机，替代 L1 同步语义（`expr.await` ≡ 直接求值）。

```zeta
// 顶层 async fn f 被 desugar 为三个产物（以 `async fn f(x: i64) -> i64` 为例）：
// 1. struct __Fut_f { state: i64, x: i64, __fut_0: __Fut_g, ... }  // 参数 + 提升字段 + 子 future 槽
// 2. impl Future for __Fut_f { fn poll(&mut self) -> Poll<i64> { ... } }  // 状态机
// 3. fn f(x: i64) -> __Fut_f { __Fut_f { state: 0, ... } }  // 构造器（zeroing 子 future 槽）

async fn g(x: i64) -> i64 { x + 1 }

async fn f() -> i64 {
    let base = 40;
    let v = g(base).await;   // await 段 k=0：首轮询 state==0，恢复轮询 state==1
    v + 1                     // 尾段 k=1：state==2
}

fn main() {
    let mut fut = f();
    let v = block_on(&mut fut);   // block_on 循环轮询直至 Ready
    println(v);                   // 43
}
```

状态机编号：await 段 `k` 占两个状态——`2k`（首轮询：初始化子 future 并 poll）、`2k+1`（恢复轮询：仅 poll 已存槽）；Ready 推进 `2k+2`，Pending 保存 `2k+1` 并返回 `Poll::Pending`；尾段（无 await 的最后一段）状态 `2k`，`return Poll::Ready(尾值)`；兜底 `Poll::Ready(0)` 不可达。

支持范围：
- async fn 参数限 `i64`，返回限 `i64` / `()`；顶层、非泛型、无递归。
- `.await` 目标：async fn 直接调用（`g(x).await`，经子 future 槽 + `self.__fut_k = g(x)`）或带类型注解的变量（`let fut: G = mk_g(41); fut.await`，G 为 `impl Future` 的类型）。
- await 位置：`let v = e.await;` / `e.await;`（语句）/ `return e.await;` / 块尾表达式 `e.await`。
- 跨 await 的 `i64` 局部变量（含 await 结果）提升为结构体字段，按数据依赖拓扑排序。

限制（显式报错）：
- 控制流块内（if/match/loop 体）await 不支持（await 仅限直线代码）；表达式中间嵌套 await（如 `a.await + b.await`）不支持。
- 递归 async fn 不支持；await 目标变量的初始化表达式仅可引用参数（不可引用跨 await 提升变量）。

验收：`tests/run-pass/async-fns.{zeta,out}`（输出 `42`/`21`/`18`/`40`/`199`：无 await 单尾段、嵌套 await、多 await 链 + 跨段变量、语句形式 await + return await、手动 Pending 恢复跨 await）；`tests/run-pass/async_await.zeta`（输出 `43`：手写状态机与 desugar 同构对照）；`tests/run-pass/manual_state_machine.zeta`（输出 `43`：手写状态机轮询示例）。

---

## 11. 智能指针

> **实现状态（2026-08-23）**：`Box<T>`（K2）/ `Rc<T>`/`Arc<T>`/`Weak<T>`（K3）为**编译器内建**
> （typecheck 特判，零新增 IR 节点，无 std 结构体定义）。`Rc<T>` 布局 = 堆 `RcInner` 的
> `T` 值区自堆首槽起（与 `Box<T>` 同构，剥层零差异）+ 尾部两计数槽（strong = 值区槽数、
> weak = +1）；`Rc<T>` 栈上 1 槽指向 RcInner，分配 `(n+2)` 个 8 字节槽。支持
> `clone`/`strong_count`/`weak_count`/`downgrade`/`try_unwrap`/`upgrade` 及 `*` 解引用、
> 字段/方法/索引自动剥层。`Gc<T>`（K4）✅ 已实现（MVP）：`Gc::new` 编译器内建 +
> `gc_region` 块生命周期（`zeta_gc_region_begin`/`zeta_gc_alloc`/`zeta_gc_escape`/
> `zeta_gc_collect`，保守标记-清除运行时 `zeta-gc-runtime`），逃逸对象 root 登记、嵌套块
> 存活链式提升、字段/方法/索引自动剥层与 `Box` 同构；块外对象永不回收（MVP 泄漏语义）。
> 以下为目标 API 参考。

```zeta
/// 堆分配（唯一所有权）
struct Box<T> {
    ptr: *mut T,
}

impl<T> Box<T> {
    fn new(value: T) -> Box<T>;
    fn leak(self) -> &'static mut T;
}

/// 引用计数（单线程）
struct Rc<T> {
    ptr: *mut RcInner<T>,
}

impl<T> Rc<T> {
    fn new(value: T) -> Rc<T>;
    fn clone(&self) -> Rc<T>;  // 增加引用计数
    fn strong_count(&self) -> usize;
    fn weak_count(&self) -> usize;
    fn downgrade(&self) -> Weak<T>;
    fn try_unwrap(self) -> Result<T, Rc<T>>;
}

/// 弱引用
struct Weak<T> {
    ptr: *mut RcInner<T>,
}

impl<T> Weak<T> {
    fn upgrade(&self) -> Option<Rc<T>>;
}

/// 原子引用计数（多线程）
struct Arc<T> {
    ptr: *mut ArcInner<T>,
}

// Arc 的 API 与 Rc 类似，但内部计数是原子的
```

---

## 12. 错误处理

```zeta
/// 标准错误 trait
trait Error {
    fn description(&self) -> &str;
    fn cause(&self) -> Option<&dyn Error>;
    fn source(&self) -> Option<&dyn Error>;
}

/// 从其他错误类型转换
trait From<T> {
    fn from(value: T) -> Self;
}

/// 自动错误转换（? 运算符使用）
trait Into<T> {
    fn into(self) -> T;
}

/// 常用错误类型
struct IoError {
    kind: IoErrorKind,
    message: String,
}

enum IoErrorKind {
    NotFound,
    PermissionDenied,
    ConnectionRefused,
    ConnectionReset,
    TimedOut,
    // ...
}
```

---

> **维护者**：Zeta Language Team  
> **License**：MIT / Apache-2.0
