// rlyeh-std/rlyeh/module.rl：标准库的**声明与导出**入口——只做两件事：
//   1. 声明子模块（`module time;` / `module io;` …）；
//   2. 导出各模块暴露的内容到根命名空间（`pub import time::duration::Duration;` …）。
//      本文件位于**顶层**，路径须写完整（无相对基准）；模块**内部**的 `pub import`
//      则写相对本模块的子模块路径即可（见 typecheck `resolve_import_path`，
//      如 `time/module.rl` 的 `pub import duration::Duration;`）。
//
// 不含任何实现。在预置加载中**最后**拼接：声明与重导出须位于全部类型之后
// （符号按顺序解析，根类型须先于子模块签名注册）。子模块经 `module <name>;`
// 相对标准库根目录解析（`<name>.rl` 或 `<name>/module.rl`）。

// Y（SH-P3-1 M1）：标记 auto-protocol 声明。`Send` / `Sync` 为并发安全标记 protocol，
// 其自动推导（M2）与并发边界检查（M3）由编译器内建谓词 `is_send_sync` 实现
// （与 Rust 一致：auto protocol 为编译器特判，不由用户 impl 触发），此处仅声明标记，
// 供用户书写 `T: Send` / `T: Sync` 约束占位。
protocol Send {}
protocol Sync {}
//
// 注：与 `core` **平级**的平铺单元（`str_ext` / `convert` / `collections` / `externs`）
// 承载根命名空间内容，**不在此声明**——它们由 rlyeh-driver/src/stdlib.rs 的 `FLAT_UNITS`
// 在 `core` 之后平铺拼接（包 `module` 壳会使 `String` / `Vec` / `HashMap` 的全名特判失配）。

// ===== 子模块拆分（2026-08-22）=====
// time/io/net/sync 的实现拆分为独立文件（rlyeh-std/rlyeh/<name>.rl），
// 由 driver 加载标准库时经模块展开（module foo; → module foo { ... }）合入。
// 声明置于文件末尾：类型符号顺序解析，根类型（String/Vec/Option/Result/
// HashMap 及编译器特判函数）必须先于子模块签名注册。
// 拆分理由：io/net/sync 基于 extern FFI 绑定 libc，模块化便于独立演进；
// String/Vec/Option/Result/HashMap 因编译器按全名特判（构造器展开、比较、
// 数组切片等），必须留在根命名空间。
module time;
module io;
// W5：future 符号须在 net/sync **之前** import，使 net/http 与 sync 模块
// （`impl Future` 自建 future 类型）收集阶段能经 use_aliases 解析裸名
// Future/Poll/Context（跨模块类型，非自建；future 不依赖 net，故可前置于 net）。
// 2026-09-18 补充：亦须**先于 `module future;`**——其子模块（executor 等）的泛型
// bound 以裸名 `Future` 引用协议，收集该模块时依赖本别名。
pub import future::interface::Future;
pub import future::poll::Poll;
pub import future::poll::Context;
// 类型短名亦须先于 `module future;`：其子模块（sleep / wait_fd / error / executor）
// 在签名的返回类型中以裸名引用（`-> Sleep` / `-> WaitFd` / `Result<_, TimeoutError>`）。
pub import future::sleep::Sleep;
pub import future::wait_fd::WaitFd;
pub import future::error::TimeoutError;
module future;
module net;
module sync;
module fs;
module thread;
module serde;
module fmt;
module process;   // D（SH-P2-2）：进程调用 / 外部工具链 FFI

// 重新导出到根命名空间，保持用户 API 不变（裸名即用，无需前缀）。
// 目录化（2026-08）：子模块按 std-lib.md §1 目标架构拆分为目录形式，
// import 路径指向类型文件完整路径（io::error::IoError 等）。
pub import serde::protocols::Serialize;
pub import time::duration::Duration;
pub import time::instant::Instant;
pub import time::system::SystemTime;
pub import io::error::IoErrorKind;
pub import io::error::IoError;
pub import io::error::Error;
// EH-5（2026-09-21）：`From` / `Into` 协议导出到根命名空间——此前仅 `io/error.rl`
// 内部可见，用户侧 `impl X: From<T>` 的协议名无法解析为规范名 `io::error::From`，
// 致 `?` 运算符的 `From` 自动转换找不到 impl（报「找不到 `io::error::From::from`
// 的可用 protocol impl」），`#[derive(Error)]` 的 `#[from]` 亦不可用。
pub import io::error::From;
pub import io::error::Into;

// EH-4（0.2.0-AA，2026-09-21）：**拥有型动态错误** `DynError`（RFC §4.4，anyhow 式便捷）。
// 实现形态为 `io/error.rl` 的**结构体** `DynError { inner: Box<dyn Error> }`（非类型别名），
// 理由：① `type` 声明仅在根单元参与收集，子模块文件里的 `type` 不注册；② 别名需写
// `Box<dyn Error>`，而 parser 的 `dyn` 只接受**单段名**（`dyn io::error::Error` 报
// `expected '>' or ',', found Colon`），根单元处裸名 `Error` 又尚不可见。
// 结构体形态反而更稳（可作字段 / 泛型实参 / `Result<_, DynError>` 的错误位）。
pub import io::error::DynError;

// EH-7（0.2.0-AA，2026-09-21）：`Result<T, DynError>` 的上下文附加组合子（RFC §4.7）。
// `Err` 分支经 `DynError::context(ctx)` 附加语义上下文（`Ok` 直通），逐层调用累积为
// `outer: inner: root` 的可读背链；`with_context` 惰性求值（`fn() -> String`，须无捕获）。
// **位置约束**：必须定义在本文件（根单元）——`collect_impl` 以「当前模块前缀 + impl 类型名」
// 构造 self 类型，写在子模块（如 `io/error.rl`）里会得到 `io::error::Result`，
// 与根单元定义的 `Result` 不匹配，方法永远找不到。
impl<T> Result<T, DynError> {
    fn context(self, msg: String) -> Result<T, DynError> {
        match self {
            Result::Ok(v) => Result::Ok(v),
            Result::Err(e) => Result::Err(e.context(msg)),
        }
    }
    fn with_context(self, f: fn() -> String) -> Result<T, DynError> {
        match self {
            Result::Ok(v) => Result::Ok(v),
            Result::Err(e) => Result::Err(e.context(f())),
        }
    }
}
pub import io::base::OpenMode;
pub import io::file::File;
pub import io::console::Stdout;
pub import io::console::Stderr;
pub import io::console::stdout;
pub import io::console::stderr;
pub import io::console::read_to_string;
pub import io::console::lines;
pub import io::base::c_str;
pub import io::file::read_file;
pub import io::file::write_file;
pub import io::file::append_file;
pub import io::console::read_line;
pub import net::byteorder::htons;
pub import net::socket::socketpair_stream;
pub import net::socket::fd_at;
pub import net::socket::send_all;
pub import net::socket::recv_some;
pub import net::byteorder::sockaddr_in4_with_layout;
pub import net::byteorder::sockaddr_in4;
pub import net::socket::tcp_connect;
pub import net::socket::hostname;
pub import net::addr::Ipv4Octets;
pub import net::addr::ipv4_octets;
pub import net::addr::SocketAddr;
pub import net::tcp::TcpListener;
pub import net::tcp::TcpStream;
pub import net::udp::UdpSocket;
pub import net::udp::UdpPacket;
pub import net::addr::Shutdown;
pub import net::http::HttpClient;
pub import net::http::Response;
pub import net::http::parse_url;
// 注意：以下均写**深层**全名（如 `sync::mutex::Mutex`），使裸名 `Mutex` 经一次
// 别名跳转即得到规范符号 `sync::mutex::Mutex`；若写成浅层 `sync::Mutex` 会因
// 别名链 `sync::Mutex → sync::mutex::Mutex` 在方法派发时停在别名名而失配。
pub import sync::mutex::Mutex;
pub import sync::mutex::MutexGuard;
pub import sync::rwlock::RwLock;
pub import sync::rwlock::RwLockReadGuard;
pub import sync::rwlock::RwLockWriteGuard;
// H-M2（SH-P0-4）：原子类型 + 内存序（裸名构造，与 sync::mutex::Mutex 同一路径约定）
pub import sync::atomic::AtomicI64;
pub import sync::atomic::Ordering;
pub import sync::condvar::Condvar;
pub import sync::barrier::Barrier;
pub import sync::channel::Channel;
pub import sync::channel::Sender;
pub import sync::channel::Receiver;
pub import sync::channel::ChannelPair;
pub import sync::channel::SendError;
pub import sync::channel::RecvError;
pub import sync::channel::TryRecvError;
pub import sync::channel::RecvAsync;
pub import sync::channel::channel;
pub import sync::channel::bounded_channel;
pub import fs::path::Path;
pub import io::nio::Interest;
pub import io::nio::Event;
pub import io::nio::Poller;
pub import io::nio::set_nonblocking;
pub import io::nio::is_nonblocking;
pub import io::sendfile::sendfile;
pub import thread::handle::Thread;
pub import thread::builder::Builder;   // Y8：线程栈定制构建器
pub import thread::ops::sleep;
pub import thread::ops::join_all;
pub import future::interface::Future;
pub import future::poll::Poll;
pub import future::poll::Context;
pub import future::executor::block_on;
pub import future::executor::timeout;
pub import future::error::TimeoutError;
pub import fmt::display::Display;
pub import fmt::debug::Debug;
pub import fmt::formatter::Formatter;

// D（SH-P2-2）：进程调用 / 外部工具链 FFI
pub import process::exec::exec;
pub import process::exec::system;
pub import process::exec::output;
pub import process::exec::exec_combined;
pub import process::output::Output;

