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
module future;
// W5：future 符号在 net/sync 之前 import，使 net/http 与 sync 模块（`impl Future`
// 自建 future 类型）收集阶段能经 use_aliases 解析裸名 Future/Poll/Context
// （跨模块类型，非自建；future 不依赖 net，故可前置于 net）。
pub import future::Future;
pub import future::Poll;
pub import future::Context;
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
pub import serde::traits::Serialize;
pub import time::duration::Duration;
pub import time::instant::Instant;
pub import time::system::SystemTime;
pub import io::error::IoErrorKind;
pub import io::error::IoError;
pub import io::error::Error;
pub import io::OpenMode;
pub import io::file::File;
pub import io::console::Stdout;
pub import io::console::Stderr;
pub import io::console::stdout;
pub import io::console::stderr;
pub import io::console::read_to_string;
pub import io::console::lines;
pub import io::c_str;
pub import io::file::read_file;
pub import io::file::write_file;
pub import io::file::append_file;
pub import io::console::read_line;
pub import net::byteorder::htons;
pub import net::socketpair_stream;
pub import net::fd_at;
pub import net::send_all;
pub import net::recv_some;
pub import net::byteorder::sockaddr_in4_with_layout;
pub import net::byteorder::sockaddr_in4;
pub import net::tcp_connect;
pub import net::hostname;
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
pub import sync::Mutex;
pub import sync::RwLock;
pub import sync::MutexGuard;
// H-M2（SH-P0-4）：原子类型 + 内存序（裸名构造，与 sync::Mutex 同一路径约定）
pub import sync::AtomicI64;
pub import sync::Ordering;
pub import sync::Condvar;
pub import sync::Barrier;
pub import sync::Sender;
pub import sync::Receiver;
pub import sync::ChannelPair;
pub import sync::RecvAsync;
pub import sync::channel;
pub import fs::path::Path;
pub import io::nio::Interest;
pub import io::nio::Event;
pub import io::nio::Poller;
pub import io::nio::set_nonblocking;
pub import io::nio::is_nonblocking;
pub import io::sendfile::sendfile;
pub import thread::Thread;
pub import thread::Builder;   // Y8：线程栈定制构建器
pub import thread::sleep;
pub import thread::join_all;
pub import future::Future;
pub import future::Poll;
pub import future::Context;
pub import future::block_on;
pub import future::timeout;
pub import future::TimeoutError;
pub import fmt::display::Display;
pub import fmt::debug::Debug;
pub import fmt::formatter::Formatter;

// D（SH-P2-2）：进程调用 / 外部工具链 FFI
pub import process::exec::exec;
pub import process::exec::system;
pub import process::exec::output;
pub import process::exec::exec_combined;
pub import process::output::Output;

