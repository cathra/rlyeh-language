//! 零拷贝文件传输：`sendfile` 系统调用封装。
//!
//! 数据在**内核态**直接从文件拷贝到 socket，全程不经过用户态缓冲区，
//! 适用于静态文件响应、大文件上传代理等场景。
//!
//! 对应 Zeta 标准库 `std::io::sendfile`：
//!
//! ```zeta
//! // 发送整个文件（offset 起至 EOF）
//! let n = sendfile(sock_fd, file_fd, 0, 0);
//! // 只发送 count 字节
//! let n = sendfile(sock_fd, file_fd, 64, 4096);
//! ```

use std::io;

use crate::nio::RawFd;

/// 平台实现：Linux 用 `sendfile(2)`，macOS/BSD 用 `sendfile(2)`（参数顺序相反），
/// 其余平台暂返回 `Unsupported`（后续可补 `TransmitFile` 等）。
mod imp {
    use super::*;
    use std::ptr;

    /// Linux：`sendfile(out_fd, in_fd, *offset, count)`，`count == 0` 表示发送到 EOF。
    #[cfg(target_os = "linux")]
    pub(super) fn sendfile(
        out_fd: RawFd,
        in_fd: RawFd,
        offset: u64,
        count: usize,
    ) -> io::Result<usize> {
        let mut off: libc::off_t = offset as libc::off_t;
        loop {
            let n = unsafe { libc::sendfile(out_fd, in_fd, ptr::from_mut(&mut off), count) };
            if n >= 0 {
                return Ok(n as usize);
            }
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(err);
        }
    }

    /// macOS/BSD：`sendfile(in_fd, out_fd, offset, *len, hdtr, flags)`，
    /// 入参 `len == 0` 表示发送到 EOF，出参 `len` 为实际发送字节数。
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly"
    ))]
    pub(super) fn sendfile(
        out_fd: RawFd,
        in_fd: RawFd,
        offset: u64,
        count: usize,
    ) -> io::Result<usize> {
        let mut len: libc::off_t = count as libc::off_t;
        loop {
            let ret = unsafe {
                libc::sendfile(
                    in_fd,
                    out_fd,
                    offset as libc::off_t,
                    ptr::from_mut(&mut len),
                    ptr::null_mut(),
                    0,
                )
            };
            if ret == 0 {
                return Ok(len as usize);
            }
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            // 非阻塞 socket 下可能 EAGAIN，但已发送部分数据
            if err.raw_os_error() == Some(libc::EAGAIN) && len > 0 {
                return Ok(len as usize);
            }
            return Err(err);
        }
    }

    /// 其余 Unix 平台暂不支持。
    #[cfg(all(
        unix,
        not(target_os = "linux"),
        not(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "dragonfly"
        ))
    ))]
    pub(super) fn sendfile(
        _out_fd: RawFd,
        _in_fd: RawFd,
        _offset: u64,
        _count: usize,
    ) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "sendfile is not supported on this platform",
        ))
    }

    /// 非 Unix 平台（Windows / WASM）暂不支持。
    #[cfg(not(unix))]
    pub(super) fn sendfile(
        _out_fd: RawFd,
        _in_fd: RawFd,
        _offset: u64,
        _count: usize,
    ) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "sendfile is not supported on this platform",
        ))
    }
}

/// 将 `in_fd` 从 `offset` 处开始的内容零拷贝发送到 `out_fd`。
///
/// - `count == 0` 表示发送到文件末尾（EOF）；
/// - 返回实际发送的字节数。
///
/// `out_fd` 通常是 socket（Linux 要求其为支持 `mmap` 语义的 fd），
/// `in_fd` 必须是可读的常规文件。
pub fn sendfile(out_fd: RawFd, in_fd: RawFd, offset: u64, count: usize) -> io::Result<usize> {
    imp::sendfile(out_fd, in_fd, offset, count)
}
