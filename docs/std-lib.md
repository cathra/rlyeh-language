# Zeta 标准库 API 规范

> 版本：v2.0  
> 最后更新：2026-08-20

## 相关文档

| 类型 | 文档 | 说明 |
|------|------|------|
| 项目总纲 | [CODEBUDDY.md](../CODEBUDDY.md) | 项目全景 |
| 设计文档 | [10_标准库规划](../design/10_标准库规划.md) | 模块规划 |
| 实现任务 | [P009](../prompts/P009_标准库核心模块.md) | 标准库实现 |

---

## 1. 标准库架构

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
底层由 `zeta-std` 绑定层实现：Linux 用 `epoll`、macOS/BSD 用 `kqueue`、其他 Unix 用 `poll`。

```zeta
/// 事件关注标志
enum Interest {
    Readable,          // 只关注可读
    Writable,          // 只关注可写
    ReadableWritable,  // 同时关注可读与可写
}

/// 就绪事件：token 用于在应用中定位对应的 fd
struct Event {
    token: u64,
    interest: Interest,
}

impl Event {
    fn is_readable(&self) -> bool;
    fn is_writable(&self) -> bool;
}

/// 事件轮询器
struct Poller {
    // 平台句柄（epoll / kqueue fd，内部持有）
}

impl Poller {
    /// 创建轮询器
    fn new() -> Result<Poller, IoError>;

    /// 注册 fd，绑定应用侧 token
    fn register(&self, fd: i32, token: u64, interest: Interest) -> Result<(), IoError>;

    /// 修改 fd 的关注事件与 token
    fn reregister(&self, fd: i32, token: u64, interest: Interest) -> Result<(), IoError>;

    /// 注销 fd
    fn deregister(&self, fd: i32) -> Result<(), IoError>;

    /// 阻塞等待就绪事件写入 events（清空后追加）；
    /// timeout = None 表示无限等待，返回本次就绪的事件个数
    fn poll(&self, events: &mut Vec<Event>, timeout: Option<Duration>) -> Result<usize, IoError>;
}

/// 设置 fd 是否为非阻塞模式
fn set_nonblocking(fd: i32, nonblocking: bool) -> Result<(), IoError>;

/// 查询 fd 是否处于非阻塞模式
fn is_nonblocking(fd: i32) -> Result<bool, IoError>;
```

典型的事件循环：

```zeta
set_nonblocking(listener.fd, true)?;
let poller = Poller::new()?;
poller.register(listener.fd, 0, Interest::Readable)?;
let mut events = Vec::new();
loop {
    poller.poll(&mut events, Some(100.ms))?;
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

> 平台差异：Linux 与 macOS/BSD 的 `sendfile(2)` 参数顺序不同，由绑定层屏蔽；
> 非 Unix 平台（Windows/WASM）当前返回 `Unsupported`，Windows 计划用 `TransmitFile` 补齐。

---

## 5. 网络模块

### 5.1 TCP

```zeta
struct TcpListener {
    fd: i32,
    addr: SocketAddr,
}

impl TcpListener {
    fn bind(addr: &str) -> Result<TcpListener, IoError>;
    fn accept(&self) -> Result<TcpStream, IoError>;
    fn local_addr(&self) -> SocketAddr;
}

struct TcpStream {
    fd: i32,
    addr: SocketAddr,
}

impl TcpStream {
    fn connect(addr: &str) -> Result<TcpStream, IoError>;
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IoError>;
    fn write(&mut self, buf: &[u8]) -> Result<usize, IoError>;
    fn peer_addr(&self) -> SocketAddr;
    fn shutdown(&self, how: Shutdown) -> Result<(), IoError>;
}
```

### 5.2 HTTP（高层封装）

```zeta
struct HttpClient {
    // ...
}

impl HttpClient {
    fn new() -> HttpClient;
    fn get(url: &str) -> Result<Response, HttpError>;
    fn post(url: &str, body: &[u8]) -> Result<Response, HttpError>;
    async fn get_async(url: &str) -> Result<Response, HttpError>;
    async fn post_async(url: &str, body: &[u8]) -> Result<Response, HttpError>;
}

struct Response {
    status: u16,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl Response {
    fn text(&self) -> String;
    fn json<T: Deserialize>(&self) -> Result<T, JsonError>;
    fn status(&self) -> u16;
}
```

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

impl Instant {
    fn now() -> Instant;
    fn duration_since(&self, earlier: Instant) -> Duration;
    fn elapsed(&self) -> Duration;
}
```

---

## 8. 格式化与打印

```zeta
/// 格式化 trait
trait Display {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>;
}

trait Debug {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>;
}

/// 打印宏
println!("Hello, {}!", name);       // 输出 + 换行
print!("Progress: {}%", pct);       // 输出不换行
eprintln!("Error: {}", msg);        // 输出到 stderr
dbg!(value);                        // 调试输出（带位置信息）

/// 格式化字符串
let s = format!("{} + {} = {}", a, b, a + b);
```

---

## 9. 序列化框架

```zeta
trait Serialize {
    fn serialize(&self, serializer: &mut Serializer) -> Result<(), SerError>;
}

trait Deserialize {
    fn deserialize(deserializer: &mut Deserializer) -> Result<Self, DeError>;
}

// JSON
mod json {
    fn to_string<T: Serialize>(value: &T) -> Result<String, JsonError>;
    fn from_str<T: Deserialize>(s: &str) -> Result<T, JsonError>;
    fn to_writer<T: Serialize>(writer: &mut Writer, value: &T) -> Result<(), JsonError>;
    fn from_reader<T: Deserialize>(reader: &mut Reader) -> Result<T, JsonError>;
}

// TOML
mod toml {
    fn to_string<T: Serialize>(value: &T) -> Result<String, TomlError>;
    fn from_str<T: Deserialize>(s: &str) -> Result<T, TomlError>;
}
```

---

## 10. 异步运行时

```zeta
trait Future {
    type Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output>;
}

enum Poll<T> {
    Ready(T),
    Pending,
}

/// 等待 Future 完成
async fn block_on<F: Future>(future: F) -> F::Output;

/// 并发等待多个 Future
async fn join_all<F: Future>(futures: Vec<F>) -> Vec<F::Output>;

/// 超时
async fn timeout<F: Future>(duration: Duration, future: F) -> Result<F::Output, TimeoutError>;

/// 并发原语
mod sync {
    fn mutex<T>(value: T) -> AsyncMutex<T>;
    fn rwlock<T>(value: T) -> AsyncRwLock<T>;
    fn notify() -> Notify;
    fn barrier(n: usize) -> Barrier;
}
```

---

## 11. 智能指针

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
