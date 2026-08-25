# Zeta 标准库 API 规范

> 版本：0.1.0  
> 最后更新：2026-08-22

> **⚠️ 实现状态**：本文为**目标标准库规范**（含规划中 API）。MVP 已实现部分位于
> `crates/zeta-std/zeta/`（**目录化模块**：`core.zeta` 根模块 + `time/`、`sync/`、`io/`、
> `net/`、`fs/` 子目录（`<name>/module.zeta` + 类型独立文件），driver 加载时经模块展开 +
> import 重新导出合入，用户侧裸名即用）。
> 根模块保留 String / Vec / HashMap / Option / Result 等编译器特判类型；未实现章节属规划（详见下方总览）。
> 教程与可运行示例见 [`guide.md`](./guide.md)；实际 API 以各模块源码为准。

## MVP 实现状态总览

> **规划阶段标注**：📋/🔧 状态任务的消解记录见 [`mvp-gaps-plan.md`](./mvp-gaps-plan.md)（阶段 G–T 已全部完成，表格末列标注归属阶段）。

| 章节 | 状态 | MVP 实际形态 | 规划阶段 |
|------|------|--------------|---------|
| §2.1 Option / §2.2 Result | ✅ 已实现 | 泛型 enum + `is_some/is_none/unwrap/unwrap_or/expect` 等 | — |
| §2.3 Iterator | ✅ 已实现（MVP 退化） | `trait Iterator { fn next(&mut self) -> Option<i64>; }`（T2 ✅，core.zeta 顶部；关联类型 `type Item` 规划——parser/typecheck 无 trait `type` 成员载体）；自定义迭代器 `impl Iterator for T` 经 for 接入（J2）；适配器 map/filter/fold/collect/take/skip 保持内建 desugar | — |
| §3.1 Vec / §3.2 HashMap / §3.3 String | ✅ 已实现（目标 API 补齐，T1 ✅） | 目标 API 清单补齐：Vec `iter`（退化元素拷贝缓冲）/`get_mut`（值拷贝）/`sort_by`（比较器闭包）；String `chars`（字节级）/`lines`/`to_uppercase`/`to_lowercase`（别名）；HashMap `iter`（退化键缓冲）/`get_mut`（值拷贝）；详见 §3.1/§3.2/§3.3 差异注记 | — |
| §4.1 File | ✅ 已实现 | `File::open/create/close` + `read_to_string/read/write/write_all/flush/metadata/size`（N1 ✅）+ 自由函数 `read_file/write_file/append_file`（`Result<T, IoError>`） | — |
| §4.2 标准输入输出 | ✅ 已实现 | `stdout`/`stderr` 模块（`write`/`writeln`/`flush`）+ stdin `read_to_string`/`lines` + `eprintln!`/`eprint!` 宏（N4 ✅） | — |
| §4.3 路径与文件系统 | ✅ 已实现 | `Path::new/join/parent/file_name/extension/exists/is_file/is_dir`（N3a ✅，`fs.zeta`）+ `fs::read_to_string/write/copy/remove_file/remove_dir_all/rename/create_dir/create_dir_all/read_dir`（N3 ✅） | — |
| §4.4 NIO | ✅ 已实现 | `Interest`/`Event`/`Poller` + `set_nonblocking`/`is_nonblocking`（R1a/R1b/R2 ✅，`io/nio.zeta` 基于 poll(2) 封装 + fcntl O_NONBLOCK，`Result<T, IoError>`） | — |
| §4.5 sendfile | ✅ 已实现 | `sendfile` 自由函数 + `File::sendfile_to`（R3 ✅，driver 注入平台内建 `__zeta_sendfile`，macOS sendfile(2) 6 参签名零拷贝） | — |
| §5.1 TCP | ✅ 已实现 | `SocketAddr`/`TcpListener`/`TcpStream` + `read/write/read_line/shutdown`（O1/O2 ✅，libc extern FFI，`Result<T, IoError>`）；旧自由函数保留兼容 | — |
| §5.2 HTTP | ✅ 已实现（同步 MVP + async 形状） | `HttpClient::get/post` + `Response::status/text` + `json::parse::<T>` 反序列化（O3 ✅，`Connection: close` 无复用）；**async 版已实现**（S3b ✅：`HttpClient::get_async/post_async`，MVP 退化同步语义，等价 get/post；事件驱动规划随 S3 事件循环 + §4.4 NIO） | — |
| §6.1 Mutex | ✅ 已实现 | `Mutex`/`RwLock` 裸 `lock/unlock/try_*` + `lock_guard()` guard 语义（作用域尾自动解锁注入）；`Condvar::wait/notify_one/notify_all` + `Barrier`（P1–P3 ✅，`sync/module.zeta` pthread extern FFI） | — |
| §6.2 Channel | ✅ 已实现 | `channel()` → `ChannelPair { tx, rx }` + `Sender::send/try_send` + `Receiver::recv/try_recv/close/iter` + **`recv_async`**（P1 ✅ + S3a ✅，`Rc<Channel>` 共享；MVP 非泛型、元素 `i64`、无界；recv_async MVP 退化阻塞语义，事件驱动规划随 R1 Poller） | — |
| §7 时间 | ✅ 已实现 | `Duration`/`Instant`（libc `clock()` extern） | — |
| §8 格式化与打印 | 🔧 部分 | **内置格式化宏已实现**（I2：`println!`/`print!`/`format!`/`dbg!` + N4 `eprintln!`/`eprint!`（stderr），`{}`/`{:?}` 占位）；**`Display`/`Debug` trait + `Formatter` 已实现**（Q3 ✅，`fmt/module.zeta`，`{}` 查 `Display::fmt`、`{:?}` 查 `Debug::fmt_debug`） | Q4 |
| §9 序列化 | 🔧 部分 | **`json::stringify`/`json::parse::<T>` 编译器内建已实现**（L2 ✅，含 HashMap + struct 反序列化）；`Serialize` trait + `#[derive(Serialize, Deserialize)]` 标记 + 手写 impl 已实现（Q1 ✅，`serde/module.zeta`）；**泛型 API 入口 `to_string`/`from_str` + 流式 `to_writer`/`from_reader` 已实现**（Q2 ✅，typecheck 内建别名/desugar）；**TOML 轻量模块已实现**（Q4 ✅，`toml::to_string`/`from_str`：基础标量/嵌套表（内联表）/数组/HashMap stringify/parse，§9.4）；`Deserialize` trait（`-> Self` 未支持）规划 | — |
| §10 异步运行时 | ✅ 已实现（MVP） | 线程（S0 ✅）、`Future`/`Poll`/`block_on`/`async fn` 状态机（S1 ✅）、`join_all`/`timeout`/`sleep`（S2 ✅）、`recv_async`/HTTP async（S3 ✅）；actor 的 `async` 方法 + `.await`/`send` 已实现（独立机制）；事件驱动 executor 与多线程调度规划（R1 Poller） | R1 |
| §11 智能指针 | ✅ 已实现 | `Box<T>`（K2，含 **`Box::leak`**（T3a ✅，返回 `*mut T` 裸指针，目标 `&'static mut T` 规划））/ `Rc<T>`/`Arc<T>`/`Weak<T>`（K3 全覆盖：`strong_count`/`weak_count`/`downgrade`/`try_unwrap`/`upgrade`，T3b ✅ 核对）/ `Gc<T>`（K4）✅ 已实现（MVP，见 §11） | — |
| §12 错误处理 | ✅ 已实现 | `Option`/`Result` + `expect/unwrap_or`；**`?` 运算符**（K1 ✅）；**`IoError`/`IoErrorKind`**（M1 ✅，`io/error.zeta`）+ **`Error` trait**（M2a ✅：`fn message(&self) -> String`）；`From`/`Into` 泛型 trait 声明可解析、`-> Self` 返回未支持（M2b ✅）；`Into::into()` 自动转换规划 | — |

> 状态标记：✅ 已实现　🔧 部分实现（注明差异）　📋 规划中（目标 API，MVP 未实现）

> **已知限制（T 阶段实测）**：typecheck 变量环境按名全局索引、无作用域隔离——同名遮蔽（如 match 臂绑定与后续 let 绑定同名）时类型互相覆盖，后续按类型分支的输出可能异常（实测 i64 200 被以 `%p` 打印为 `0xc8`）；建议避免同名变量遮蔽（作用域栈重构规划）。

---

## 相关文档

| 类型 | 文档 | 说明 |
|------|------|------|
| 项目总纲 | [CODEBUDDY.md](../CODEBUDDY.md) | 项目全景 |
| 设计文档 | [10_标准库规划](design/10_标准库规划.md) | 模块规划（目标架构） |
| 实现纪要 | [附录 A](#附录-a实现纪要)（原 P009，已归档至 [design/prompts/](design/prompts/)） | 标准库落地状态 |
| 剩余任务计划 | [mvp-gaps-plan.md](./mvp-gaps-plan.md) §3b | 阶段 M–T 开发计划（本表📋/🔧章节的规划归属） |

---

## 1. 标准库架构

> **实现状态（2026-08-23）**：已完成**目录化模块拆分**（对齐下述目标架构的目录 + 类型独立文件
> 形式；`pub import` 以根模块 `import` 重新导出等价实现）。实际布局为
> `zeta-std/zeta/core.zeta`（根模块：String / Vec / HashMap / Option / Result + 全部 extern 声明 +
> `module` 声明 + import 重新导出，指向各类型文件完整路径）+ 子目录：
> `time/`（Duration / Instant）、`sync/`（pthread 锁）、`io/`（module.zeta 聚合 OpenMode/c_str +
> error.zeta 错误类型 + file.zeta 文件对象 + console.zeta 控制台）、`net/`（module.zeta 聚合 socket
> 自由函数 + byteorder.zeta 字节打包 + addr.zeta 地址 + tcp.zeta 流与监听 + http.zeta HTTP 客户端）、
> `fs/`（module.zeta 聚合 fs 函数 + path.zeta 路径对象）。
> **符号完整路径随拆分变更**：如 `io::IoError` → `io::error::IoError`、`net::SocketAddr` →
> `net::addr::SocketAddr`、`net::TcpStream` → `net::tcp::TcpStream`、`fs::Path` → `fs::path::Path`；
> 用户侧裸名 API 不变（core.zeta import 重新导出）。下述**目标架构**（规划：按 crate 目录 + 各类型
> 独立文件 + `pub import` 重导出）中 alloc/collections/fmt/serde/async 等目录为规划内容。

```
zeta-std/
├── core/           ← 最核心的类型和 trait（无依赖）
│   ├── module.zeta
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
│   ├── module.zeta
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

> **已实现（T2 ✅，MVP 退化，`core.zeta` 顶部）**：`trait Iterator { fn next(&mut self) -> Option<i64>; }`——关联类型 `type Item` 与默认方法规划（parser/typecheck 无 trait `type` 成员载体，S1a 已验证，元素固定 i64）；自定义迭代器 `impl Iterator for T` 后经 `for` 接入（J2 检测 next() 方法，inherent 或 trait impl 均可）；适配器 `map`/`filter`/`fold`/`collect`/`take`/`skip` 保持编译器内建 desugar（未迁移到 trait 默认方法，避免重构风险）。泛型元素迭代器（如 `StdinLines` 返回 `Option<String>`）仍走方法式接入。

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

> **MVP 已实现（T1a ✅，`core.zeta`）**：目标 API 中 `push`/`pop`/`get`/`len`/`is_empty`/`sort`/`binary_search`/`contains`/`find`/`first`/`last`/`reverse`/`swap`/`remove`/`slice` 已有 ✅。**T1a 新增**：`iter`（MVP 退化——返回元素值拷贝缓冲（快照），目标 `Iter<'_, T>` 借用迭代器规划）、`get_mut`（值拷贝，目标 `Option<&mut T>` 引用语义规划）、`sort_by`（比较器 `fn(T, T) -> i64` 三态（负/零/正），目标 `Fn(&T, &T) -> Ordering` 规划；选择排序 O(n²) 非稳定）。`iter_mut` 规划。

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

> **MVP 已实现（T1c ✅，`core.zeta`）**：`new`/`with_capacity`/`insert`/`get`/`remove`/`contains_key`/`len`/`is_empty`/`keys`/`values`/`clear`/`cap` 已有 ✅。**T1c 新增**：`iter`（MVP 退化——返回键缓冲，与 `keys` 同构（可配 `values()` 配对），目标 `Iter<'_, K, V>` 键值对迭代器规划）、`get_mut`（值拷贝，目标 `Option<&mut V>` 引用语义规划）。
>
> **实现注记（Robin Hood 线性探测）**：内部为 7 槽结构——`keys`/`vals`/`states`（0=空 1=占用 2=墓碑）/`len`/`used`/`cap`（2 的幂，位掩码定位）/`dist`（每槽键探测距离数组）。`insert` 与 `grow` 重插均执行「探测 + 交换」（穷者让位），链上键距离非减；`find` 以 `dist[idx] < d` 提前终止（O(1) 判不存在，无需再哈希）。负载因子 `used/cap >= 7/8` 时翻倍扩容（较 1/2 表小一半、扩容总量减半；早退控住高负载探测）。键限 `i64`（Knuth 乘法散列）/`String`（typecheck 特判展开 djb2 内容哈希）。`grow` 重插必须交换式——纯线性重插会破坏距离不变量导致 `find` 早退假阴性。

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

> **MVP 已实现（T1b ✅，`core.zeta`）**：`new`/`from`/`push`/`push_str`/`len`/`split`（返回 `Vec<String>`）/`replace`/`trim`/`contains`/`starts_with`/`ends_with`/`find`/`substring`/`to_upper`/`to_lower` 已有 ✅。**T1b 新增**：`chars`（MVP 字节级——逐字节 i64 列表（字符 = 字节，与 to_upper / 索引步长 1 字节一致），目标 `Chars` 迭代器 + UTF-8 码点解码规划）、`lines`（委托 `split("\n")` 返回 `Vec<String>`，`\r\n` 行尾 `\r` 保留，目标 `Lines` 迭代器规划）、`to_uppercase`/`to_lowercase`（`to_upper`/`to_lower` 的 API 别名，ASCII 语义，Unicode 全角转换规划）。

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
module stdin {
    fn read_line() -> Result<String, IoError>;
    fn read_to_string() -> Result<String, IoError>;
    fn lines() -> Lines;
}

module stdout {
    fn write(s: &str) -> Result<(), IoError>;
    fn writeln(s: &str) -> Result<(), IoError>;
    fn flush() -> Result<(), IoError>;
}

module stderr {
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
module fs {
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
module sendfile {
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

> O 阶段（2026-08）✅：TCP 对象化 + HTTP 同步 MVP 已实现。实现方式为 `net/` 子目录（`net/module.zeta` 聚合 socket 自由函数 + `net/byteorder.zeta` 字节打包 + `net/addr.zeta` 地址 + `net/tcp.zeta` 流与监听 + `net/http.zeta` HTTP 客户端）直接 libc extern FFI（无 Rust 绑定层），`socklen_t` 以 8 字节小端缓冲传递；平台自适应 sockaddr_in 布局（macOS `sin_len` 头 vs Linux `sin_family`，`__zeta_target_os()` 区分）；WASI（码 5）网络函数短路返回 `Err`。符号完整路径见各文件（如 `net::addr::SocketAddr`）；示例见 `tests/run-pass/tcp_addr.zeta`、`tcp_echo.zeta` 与 `crates/zeta-driver/tests/net_http_test.rs`。

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

> MVP 语义：每次请求新建连接 + `Connection: close`（无连接复用）；`json::parse::<T>` 直接对 `r.text()` 反序列化（返回裸 `T` 非 `Result`）。**async 版已实现（S3b ✅）**：`get_async`/`post_async` 与 get/post 同签名、MVP 退化为同步语义（事件驱动版规划随 S3 事件循环 + §4.4 NIO 接入，io_uring/epoll 注册 + Future 挂起）。

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

> **MVP 已实现（P1 ✅ + S3a ✅，`sync/module.zeta`）**：目标 API 为泛型 + `Arc` 无锁队列；MVP 为非泛型 `i64` 元素 + `Rc<Channel>` 共享（Mutex + Condvar 队列），构造为 `let p = channel(); p.tx / p.rx`（`ChannelPair` 结构体含 `tx`/`rx` 槽，未提供多元组返回）。差异：`try_send` 无界队列恒 `true`；`recv`/`try_recv` 返回 `Option<i64>`（空/close 后 `None`）；**`recv_async` 已实现（S3a ✅）——MVP 退化阻塞语义（等价 `recv`），事件驱动版规划随 R1 Poller + 事件循环**（跨线程数据流 MVP 不可测：线程入口零参数 + `Rc` 非线程安全，文档化）。

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
> （std `fmt/module.zeta`，`core.zeta` `module fmt;` + `import fmt::{Display, Debug, Formatter}`）。
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
> 配套修复：模块内 trait/impl 方法签名收集阶段即 `resolve_ast_type`，import 段尚未注册——
> `resolve_full_name` 加当前模块前缀回退 + `collect_item_decls`/`collect_mod_types_inner`/
> `check_item` 的 ModDecl 分支设置 `module_prefix`（模块内短名按 `module::Name` 定位）。

---

## 9. 序列化框架

> **实现状态（2026-08-25）**：🔧 部分（Q1–Q4 ✅）。`json` 模块的 `stringify` / `parse` 已实现（L2 ✅，编译器内建 desugar，见 §9.1）；**`Serialize` trait + `#[derive(Serialize, Deserialize)]` 标记 + struct 反序列化已实现（Q1 ✅，2026-08，§9.1b）**：`serde/module.zeta` 定义 `trait Serialize { fn to_json(&self) -> String; }`（自定义类型可手写 impl 并经 `to_json()` 调用，内建类型默认 impl 为声明性——MVP 内建类型方法调用不走 trait impl 查找，序列化经 `json::stringify` 特判）；`#[derive(...)]` 语法经 lexer `Pound` + parser 特判解析（`AstStructDecl.derive`）；`json::parse::<T>` 支持 struct（字段名匹配、顺序无关、缺失字段零值、未知字段忽略、嵌套 struct）。**泛型 API 入口 + 流式 writer/reader 已实现（Q2 ✅，2026-08，§9.2）**：`json::to_string(v)` ≡ `json::stringify(v)`、`json::from_str::<T>(s)` ≡ `json::parse::<T>(s)`（typecheck 内建别名；`T: Serialize`/`T: Deserialize` trait bound 未支持——MVP 无泛型 trait 约束，签名降级为无 bound turbofish 形式）；`json::to_writer(w, v)` → `w.write_all(json::stringify(v))`（返回 `Result<i64, io::error::IoError>`）、`json::from_reader::<T>(r)` → `json::parse::<T>(r.read_to_string().unwrap())`（读失败经 `unwrap` 死循环 MVP 语义；首参须 `File`/`&File`/`&mut File`，TcpStream 留待流式 read_all 方法化）。`Deserialize` trait（`-> Self` 返回自身类型未支持，见 mvp-gaps-plan.md M2b）与 TOML 模块仍规划。

### 9.1 JSON（L2 ✅，编译器内建）

```zeta
module json {
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
// Q1a：Serialize trait 定义（std serde/module.zeta；core.zeta 全局 import serde::Serialize）
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

module json {
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
module toml {
    fn to_string<T: Serialize>(value: &T) -> Result<String, TomlError>;
    fn from_str<T: Deserialize>(s: &str) -> Result<T, TomlError>;
}
// 已实现（Q4 ✅，2026-08，§9.4 轻量 MVP，typecheck 内建接线，无 std 模块文件）：
//   toml::to_string(v)  -> String   // ≡ toml::stringify(v)，无 bound、类型由实参推断
//   toml::from_str::<T>(s) -> T     // ≡ toml::parse::<T>(s)，无 bound、turbofish 指定
//   MVP 注：`T: Serialize`/`T: Deserialize` bound 未支持（无泛型 trait 约束）；
//   无 `TomlError`（非 Result 包装，非法输入给默认值）；紧凑输出（TOML 语法合法）——
//   标准 TOML 的 `key = value` 空格形式、`[section]` 行式子表、注释、多行字符串规划中。
//   详见 §9.4。
```

### 9.4 TOML（Q4 ✅，2026-08，轻量 MVP）

```zeta
// toml::to_string / toml::from_str（typecheck 内建 desugar，同 §9.1 模式）
struct Config { name: String, enabled: bool, scores: Vec<i64>, point: Point }
let c = Config { name: String::from("z"), enabled: true, scores: vec![1, 2],
                 point: Point { x: 7, y: 9 } };
let s = toml::to_string(c);
// s = "name=\"z\"\nenabled=true\nscores=[1,2]\npoint={x=7,y=9}"
// （顶层结构体 → 多行 `f1=v1\nf2=v2`；嵌套结构体字段 → 内联表 `{x=7,y=9}`；
//   数组/Vec → `[e1,e2]`；HashMap → `{"k"=v,...}`（键带引号，限 i64/String）；
//   标量：i64 → 十进制；bool → `true`/`false`；String → `"` + json_escape + `"`，
//   TOML 基本转义与 JSON 一致，复用 core.zeta `json_escape`/`json_unescape`）
let c2 = toml::from_str::<Config>(s);          // round-trip（字段序无关、缺失零值、未知忽略）
let v = toml::from_str::<Vec<i64>>("[1,2,3]"); // 数组剥 [ ] + split(",")，元素限标量
let m = toml::from_str::<HashMap<String, i64>>("{\"a\"=1,\"b\"=2}");  // 内联表剥 { } + find("=")
// 嵌套结构体字段值为内联表 `{...}`（parse 剥 { } + split(",") 递归）
```

> Q4 语义说明：与 json（L2）同构——typecheck 内建（`check_toml_stringify` /
> `check_toml_parse`），AST 层 desugar 为零新增 IR 节点。stringify 顶层结构体输出多行
> `key=value`（标准 TOML 顶层键值对形态），嵌套结构体字段输出内联表 `{...}`（MVP 用
> 内联表；`[section]` 行式子表规划中）；parse 的 struct 分支为行式 `split("\n")` /
> 内联表 `split(",")` 两形态（`inline` 参数区分），字段名比较 if-else 链逐字段 `Assign`。
> MVP 限制：紧凑输出（TOML 语法合法，parse 接受与 stringify 一致的紧凑文本；
> 标准 TOML 的 `key = value` 空格形式、注释、多行字符串规划中）；数组反序列化仅
> `Vec<T>`（元素限 i64/bool/String；数组类型 `[T; N]` 与 f64 报 Unsupported）；HashMap
> 键/值限标量（嵌套内联表值报 Unsupported）；struct 字段值含逗号经 split 分段不可靠
> （与 §9.1 json 一致）；无 std 模块文件（typecheck 内建接线）。

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
module sync {
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
（`crates/zeta-std/zeta/thread/module.zeta`）。

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
- MVP 无 TLS / 线程局部状态需求（S0e ✅）。
- **默认栈大小（S0e ✅）**：`pthread_create` 传 `attr = NULL` 使用系统默认栈——Linux（glibc）新线程默认约 8MB（受 `ulimit -s` 约束）；macOS 新线程默认约 512KB。`thread::Builder::stack_size` 定制规划中；需要大栈的深递归场景 Linux 下经 `ulimit -s` 生效。
- 线程内存模型（S0e ✅）：每线程独立栈 + 独立寄存器上下文，堆共享；共享数据须经同步原语（Mutex/Condvar/Channel）保证可见性，MVP 无内存模型排序保证，数据竞争 UB 由调用方负责（与 C 并发内存模型一致）。

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

## 附录 A：实现纪要

> **说明**：本节提炼自开发任务书（原 `prompts/P009`，2026-08-24 归档至 [`design/prompts/`](design/prompts/)），
> 记录标准库核心模块的落地形态与性能验收指标。API 设计见正文各章节。

### A.1 标准库核心模块（对应 P009，2026-08-20 ✅）

- **实现形态**：`crates/zeta-std/zeta/` 目录化模块——`core.zeta` 根模块（Option/Result/String/Vec/
  HashMap 编译器特判类型）+ `time/`、`sync/`、`io/`、`net/`、`fs/` 子目录（`<name>/module.zeta` +
  类型独立文件）；driver 加载时经模块展开 + import 重新导出合入，用户侧裸名即用。
- **纯 Zeta 实现**：Option/Result 为 `core.zeta` 中泛型 enum（`is_some`/`is_none`/`unwrap`/
  `unwrap_or`/`expect` 等）；Vec/HashMap/String 为编译器特判类型 + 目标 API 补齐（见正文 §3 差异注记）。
- **性能验收指标**（P009 基准，全部通过）：

| 指标 | 目标 | 状态 |
|------|------|------|
| Vec | 100 万次 `push` < 50ms | ✅ |
| String | 拼接 10KB < 1ms | ✅ |
| HashMap | 10 万次插入 + 查找 < 100ms | ✅ |
| Channel | 吞吐量 > 1M msg/s（单线程） | ✅ |

- **后续扩展**：剩余规划模块（HashSet、路径/文件系统、异步事件驱动等）消解计划见
  [`mvp-gaps-plan.md`](./mvp-gaps-plan.md)。

---

> **维护者**：Zeta Language Team  
> **License**：MIT / Apache-2.0
