//! 非阻塞 IO（NIO）与零拷贝传输（sendfile）支持。
//!
//! 对应 Rlyeh 标准库 `std::nio` 模块：
//!
//! | Rlyeh API            | 说明                             | 绑定实现        |
//! |---------------------|----------------------------------|-----------------|
//! | `Interest`          | 事件关注标志                     | `event.rs`      |
//! | `Event`             | 就绪事件（token + interest）     | `event.rs`      |
//! | `Poller`            | 事件轮询器（epoll/kqueue/poll）  | `poller.rs`     |
//! | `set_nonblocking`   | 设置 fd 非阻塞模式               | `nonblocking.rs`|
//! | `is_nonblocking`    | 查询 fd 非阻塞模式               | `nonblocking.rs`|
//! | `sendfile`          | 内核零拷贝文件传输               | `sendfile.rs`   |
//!
//! 典型 NIO 用法：
//!
//! ```rlyeh
//! set_nonblocking(listener.fd, true)?;
//! let poller = Poller::new()?;
//! poller.register(listener.fd, 0, Interest::Readable)?;
//! loop {
//!     let mut events = Vec::new();
//!     poller.poll(&mut events, Some(100.ms))?;
//!     for e in events {
//!         match e.token {
//!             0 => accept_connection(listener)?,
//!             _ => handle_conn(e.token, e.interest)?,
//!         }
//!     }
//! }
//! ```

pub mod event;
pub mod nonblocking;
pub mod poller;
pub mod sendfile;

/// fd 类型（Rlyeh 语言中为 `i32`）。
pub type RawFd = i32;

pub use event::{Event, Interest};
pub use nonblocking::{is_nonblocking, set_nonblocking};
pub use poller::Poller;
pub use sendfile::sendfile;
