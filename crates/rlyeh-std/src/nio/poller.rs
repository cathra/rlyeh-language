//! NIO 事件轮询器：`epoll`（Linux）/ `kqueue`（macOS/BSD）/ `poll`（其他 Unix）。
//!
//! 平台无关接口，对应 Rlyeh 标准库 `std::nio::Poller`：
//!
//! ```rlyeh
//! let poller = Poller::new()?;
//! poller.register(sock.fd, 1, Interest::Readable)?;
//! let mut events = Vec::new();
//! let n = poller.poll(&mut events, Some(100.ms))?;   // 最多等待 100ms
//! for e in events { process(e.token, e.interest); }
//! ```

use std::io;
use std::time::Duration;

use super::event::{Event, Interest};
use super::RawFd;

/// NIO 事件轮询器。
///
/// 支持注册 / 重新注册 / 注销 fd，并阻塞等待就绪事件。
/// 与 [`super::nonblocking::set_nonblocking`] 配合使用即可实现
/// 事件驱动（NIO）编程模型。
pub struct Poller(imp::Poller);

impl Poller {
    /// 创建轮询器（底层为 `epoll` / `kqueue`，进程内不共享）。
    pub fn new() -> io::Result<Self> {
        imp::Poller::new().map(Self)
    }

    /// 注册 `fd`，绑定应用侧 `token`，关注 `interest` 对应的事件。
    ///
    /// 若 `fd` 已注册，返回 `AlreadyExists`（Linux）等错误，请改用
    /// [`Self::reregister`]。
    pub fn register(&self, fd: RawFd, token: u64, interest: Interest) -> io::Result<()> {
        self.0.register(fd, token, interest)
    }

    /// 修改 `fd` 的关注事件与 token。
    pub fn reregister(&self, fd: RawFd, token: u64, interest: Interest) -> io::Result<()> {
        self.0.reregister(fd, token, interest)
    }

    /// 注销 `fd`，此后不再报告其事件。
    pub fn deregister(&self, fd: RawFd) -> io::Result<()> {
        self.0.deregister(fd)
    }

    /// 阻塞等待就绪事件，写入 `events`（清空后追加）。
    ///
    /// - `timeout == Some(d)`：至多等待 `d`，超时返回 `Ok(0)`；
    /// - `timeout == None`：无限等待。
    ///
    /// 返回本次就绪的事件个数。
    pub fn poll(&self, events: &mut Vec<Event>, timeout: Option<Duration>) -> io::Result<usize> {
        self.0.poll(events, timeout)
    }
}

// ---------------------------------------------------------------------------
// Linux：epoll
// ---------------------------------------------------------------------------
#[cfg(target_os = "linux")]
mod imp {
    use super::*;

    pub(super) struct Poller {
        fd: libc::c_int,
    }

    impl Poller {
        pub(super) fn new() -> io::Result<Self> {
            let fd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Self { fd })
        }

        pub(super) fn register(&self, fd: RawFd, token: u64, interest: Interest) -> io::Result<()> {
            self.ctl(libc::EPOLL_CTL_ADD, fd, token, interest)
        }

        pub(super) fn reregister(
            &self,
            fd: RawFd,
            token: u64,
            interest: Interest,
        ) -> io::Result<()> {
            self.ctl(libc::EPOLL_CTL_MOD, fd, token, interest)
        }

        pub(super) fn deregister(&self, fd: RawFd) -> io::Result<()> {
            self.ctl(libc::EPOLL_CTL_DEL, fd, 0, Interest::Readable)
        }

        pub(super) fn poll(
            &self,
            events: &mut Vec<Event>,
            timeout: Option<Duration>,
        ) -> io::Result<usize> {
            let timeout_ms = timeout.map(|d| d.as_millis() as i32).unwrap_or(-1);
            let capacity = events.capacity().max(32);
            let mut raw: Vec<libc::epoll_event> = (0..capacity)
                .map(|_| libc::epoll_event { events: 0, u64: 0 })
                .collect();
            loop {
                let n = unsafe {
                    libc::epoll_wait(self.fd, raw.as_mut_ptr(), capacity as i32, timeout_ms)
                };
                if n >= 0 {
                    events.clear();
                    events.extend(
                        raw[..n as usize]
                            .iter()
                            .map(|e| Event::new(e.u64, Self::from_epoll_flags(e.events))),
                    );
                    return Ok(events.len());
                }
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(err);
            }
        }

        fn ctl(
            &self,
            op: libc::c_int,
            fd: RawFd,
            token: u64,
            interest: Interest,
        ) -> io::Result<()> {
            let mut ev = libc::epoll_event {
                events: Self::to_epoll_flags(interest),
                u64: token,
            };
            let ret = unsafe { libc::epoll_ctl(self.fd, op, fd, &mut ev) };
            if ret < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }

        fn to_epoll_flags(interest: Interest) -> u32 {
            let mut f = 0u32;
            if interest.is_readable() {
                f |= libc::EPOLLIN;
            }
            if interest.is_writable() {
                f |= libc::EPOLLOUT;
            }
            f
        }

        fn from_epoll_flags(flags: u32) -> Interest {
            // EPOLLERR / EPOLLHUP 等异常位同样视作「可读」，交由应用层 read 处理
            let readable = flags & (libc::EPOLLIN as u32) != 0;
            let writable = flags & (libc::EPOLLOUT as u32) != 0;
            match (readable, writable) {
                (true, true) => Interest::ReadableWritable,
                (true, false) => Interest::Readable,
                (false, true) => Interest::Writable,
                (false, false) => Interest::Readable,
            }
        }
    }

    impl Drop for Poller {
        fn drop(&mut self) {
            unsafe {
                libc::close(self.fd);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// macOS / BSD：kqueue
// ---------------------------------------------------------------------------
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
mod imp {
    use super::*;
    use std::ptr;

    pub(super) struct Poller {
        fd: libc::c_int,
    }

    impl Poller {
        pub(super) fn new() -> io::Result<Self> {
            let fd = unsafe { libc::kqueue() };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Self { fd })
        }

        pub(super) fn register(&self, fd: RawFd, token: u64, interest: Interest) -> io::Result<()> {
            self.apply(fd, token, interest, libc::EV_ADD | libc::EV_ENABLE)
        }

        pub(super) fn reregister(
            &self,
            fd: RawFd,
            token: u64,
            interest: Interest,
        ) -> io::Result<()> {
            // kqueue 按 (ident, filter) 唯一，改关注前先删除旧 filter（忽略未注册错误）
            let _ = self.delete_filters(fd);
            self.apply(fd, token, interest, libc::EV_ADD | libc::EV_ENABLE)
        }

        pub(super) fn deregister(&self, fd: RawFd) -> io::Result<()> {
            self.delete_filters(fd)
        }

        pub(super) fn poll(
            &self,
            events: &mut Vec<Event>,
            timeout: Option<Duration>,
        ) -> io::Result<usize> {
            let capacity = events.capacity().max(32);
            let mut raw: Vec<libc::kevent> =
                (0..capacity).map(|_| Self::kevent_of(0, 0, 0, 0)).collect();
            let timeout_ts = timeout.map(|d| libc::timespec {
                tv_sec: d.as_secs() as libc::time_t,
                tv_nsec: d.subsec_nanos() as libc::c_long,
            });
            let timeout_ptr: *const libc::timespec = match &timeout_ts {
                Some(t) => t as *const libc::timespec,
                None => ptr::null(),
            };
            loop {
                let n = unsafe {
                    libc::kevent(
                        self.fd,
                        ptr::null(),
                        0,
                        raw.as_mut_ptr(),
                        capacity as i32,
                        timeout_ptr,
                    )
                };
                if n >= 0 {
                    events.clear();
                    for k in &raw[..n as usize] {
                        let interest = if k.filter == libc::EVFILT_READ {
                            Interest::Readable
                        } else {
                            Interest::Writable
                        };
                        events.push(Event::new(k.udata as usize as u64, interest));
                    }
                    return Ok(events.len());
                }
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(err);
            }
        }

        fn apply(&self, fd: RawFd, token: u64, interest: Interest, flags: u16) -> io::Result<()> {
            let mut changes = Vec::new();
            if interest.is_readable() {
                changes.push(Self::kevent_of(fd, libc::EVFILT_READ, flags, token));
            }
            if interest.is_writable() {
                changes.push(Self::kevent_of(fd, libc::EVFILT_WRITE, flags, token));
            }
            if changes.is_empty() {
                return Ok(());
            }
            let ret = unsafe {
                libc::kevent(
                    self.fd,
                    changes.as_ptr(),
                    changes.len() as i32,
                    ptr::null_mut(),
                    0,
                    ptr::null(),
                )
            };
            if ret < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }

        fn delete_filters(&self, fd: RawFd) -> io::Result<()> {
            let changes = [
                Self::kevent_of(fd, libc::EVFILT_READ, libc::EV_DELETE, 0),
                Self::kevent_of(fd, libc::EVFILT_WRITE, libc::EV_DELETE, 0),
            ];
            let ret = unsafe {
                libc::kevent(
                    self.fd,
                    changes.as_ptr(),
                    changes.len() as i32,
                    ptr::null_mut(),
                    0,
                    ptr::null(),
                )
            };
            if ret < 0 {
                let err = io::Error::last_os_error();
                // ENOENT：filter 未注册，视为注销成功
                if err.raw_os_error() != Some(libc::ENOENT) {
                    return Err(err);
                }
            }
            Ok(())
        }

        fn kevent_of(fd: RawFd, filter: i16, flags: u16, token: u64) -> libc::kevent {
            libc::kevent {
                ident: fd as libc::uintptr_t,
                filter,
                flags,
                fflags: 0,
                data: 0,
                udata: token as usize as *mut libc::c_void,
            }
        }
    }

    impl Drop for Poller {
        fn drop(&mut self) {
            unsafe {
                libc::close(self.fd);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 其他 Unix：poll（注册表 + pollfd 重建）
// ---------------------------------------------------------------------------
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
mod imp {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    pub(super) struct Poller {
        registry: Mutex<HashMap<RawFd, (u64, Interest)>>,
    }

    impl Poller {
        pub(super) fn new() -> io::Result<Self> {
            Ok(Self {
                registry: Mutex::new(HashMap::new()),
            })
        }

        pub(super) fn register(&self, fd: RawFd, token: u64, interest: Interest) -> io::Result<()> {
            self.registry.lock().unwrap().insert(fd, (token, interest));
            Ok(())
        }

        pub(super) fn reregister(
            &self,
            fd: RawFd,
            token: u64,
            interest: Interest,
        ) -> io::Result<()> {
            self.register(fd, token, interest)
        }

        pub(super) fn deregister(&self, fd: RawFd) -> io::Result<()> {
            self.registry.lock().unwrap().remove(&fd);
            Ok(())
        }

        pub(super) fn poll(
            &self,
            events: &mut Vec<Event>,
            timeout: Option<Duration>,
        ) -> io::Result<usize> {
            let registry = self.registry.lock().unwrap();
            let mut pollfds: Vec<libc::pollfd> = registry
                .iter()
                .map(|(&fd, &(_, interest))| libc::pollfd {
                    fd,
                    events: to_poll_flags(interest),
                    revents: 0,
                })
                .collect();
            let timeout_ms = timeout.map(|d| d.as_millis() as i32).unwrap_or(-1);
            loop {
                let n = unsafe {
                    libc::poll(
                        pollfds.as_mut_ptr(),
                        pollfds.len() as libc::nfds_t,
                        timeout_ms,
                    )
                };
                if n >= 0 {
                    events.clear();
                    for p in &pollfds {
                        if p.revents == 0 {
                            continue;
                        }
                        if let Some(&(token, interest)) = registry.get(&p.fd) {
                            events.push(Event::new(token, filter_interest(p.revents, interest)));
                        }
                    }
                    return Ok(events.len());
                }
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(err);
            }
        }
    }

    fn to_poll_flags(interest: Interest) -> libc::c_short {
        let mut f = 0 as libc::c_short;
        if interest.is_readable() {
            f |= libc::POLLIN;
        }
        if interest.is_writable() {
            f |= libc::POLLOUT;
        }
        f
    }

    fn filter_interest(revents: libc::c_short, registered: Interest) -> Interest {
        // POLLERR / POLLHUP / POLLNVAL 视作可读，交由应用层 read 处理
        let readable =
            revents & (libc::POLLIN | libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0;
        let writable = revents & libc::POLLOUT != 0;
        match (readable, writable) {
            (true, true) => Interest::ReadableWritable,
            (true, false) => Interest::Readable,
            (false, true) => Interest::Writable,
            (false, false) => registered,
        }
    }
}

// ---------------------------------------------------------------------------
// 非 Unix：暂不支持
// ---------------------------------------------------------------------------
#[cfg(not(unix))]
mod imp {
    use super::*;

    pub(super) struct Poller;

    impl Poller {
        pub(super) fn new() -> io::Result<Self> {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Poller is not supported on this platform",
            ))
        }

        pub(super) fn register(
            &self,
            _fd: RawFd,
            _token: u64,
            _interest: Interest,
        ) -> io::Result<()> {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Poller is not supported on this platform",
            ))
        }

        pub(super) fn reregister(
            &self,
            _fd: RawFd,
            _token: u64,
            _interest: Interest,
        ) -> io::Result<()> {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Poller is not supported on this platform",
            ))
        }

        pub(super) fn deregister(&self, _fd: RawFd) -> io::Result<()> {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Poller is not supported on this platform",
            ))
        }

        pub(super) fn poll(
            &self,
            _events: &mut Vec<Event>,
            _timeout: Option<Duration>,
        ) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Poller is not supported on this platform",
            ))
        }
    }
}
