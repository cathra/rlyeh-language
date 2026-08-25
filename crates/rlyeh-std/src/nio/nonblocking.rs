//! 非阻塞模式设置：`fcntl` 的 `O_NONBLOCK` 封装。
//!
//! 对应 Rlyeh 标准库 `std::nio` 中的 `set_nonblocking` / `is_nonblocking`。

use std::io;

use crate::nio::RawFd;

/// 平台无关实现（Unix 用 `fcntl`，其余平台返回 `Unsupported`）。
mod imp {
    use super::*;

    #[cfg(unix)]
    pub(super) fn set_nonblocking(fd: RawFd, nonblocking: bool) -> io::Result<()> {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        let new_flags = if nonblocking {
            flags | libc::O_NONBLOCK
        } else {
            flags & !libc::O_NONBLOCK
        };
        let ret = unsafe { libc::fcntl(fd, libc::F_SETFL, new_flags) };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    #[cfg(unix)]
    pub(super) fn is_nonblocking(fd: RawFd) -> io::Result<bool> {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(flags & libc::O_NONBLOCK != 0)
    }

    #[cfg(not(unix))]
    pub(super) fn set_nonblocking(_fd: RawFd, _nonblocking: bool) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "set_nonblocking is not supported on this platform",
        ))
    }

    #[cfg(not(unix))]
    pub(super) fn is_nonblocking(_fd: RawFd) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "is_nonblocking is not supported on this platform",
        ))
    }
}

/// 设置 fd 是否为非阻塞模式。
///
/// 非阻塞模式下，当读写无法立即完成时立即返回 `WouldBlock` 错误，
/// 配合 [`crate::nio::Poller`] 实现事件驱动的 NIO 编程。
pub fn set_nonblocking(fd: RawFd, nonblocking: bool) -> io::Result<()> {
    imp::set_nonblocking(fd, nonblocking)
}

/// 查询 fd 当前是否处于非阻塞模式。
pub fn is_nonblocking(fd: RawFd) -> io::Result<bool> {
    imp::is_nonblocking(fd)
}
