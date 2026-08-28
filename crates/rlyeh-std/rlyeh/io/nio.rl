// io/nio.rl：非阻塞 IO（Interest / Event / Poller，std-lib.md §4.4）。
// R 阶段（2026-08，R1a/R1b/R2）：
// - 事件轮询基于 poll(2)（POSIX 统一，macOS/Linux/FreeBSD 均可用）实现；
//   规划中"Linux epoll / macOS kqueue"为高性能替代，MVP 以 poll 先行
//   （语义一致：register/deregister/reregister/poll + POLLIN/POLLOUT 映射）。
// - 非阻塞模式经 fcntl(F_GETFL/F_SETFL) 设置 O_NONBLOCK（macOS/Linux 均为 0x4）。
// - 底层 extern（fcntl/poll）声明于根模块 core.rl 的 extern 集中区。
// 注意：WASI（码 5）无 poll/fcntl 语义，相关函数短路返回 Err（L4a 禁用文档化）。

// R1a：事件关注标志（std-lib.md §4.4）。
enum Interest {
    Readable,
    Writable,
    ReadableWritable,
}

// R1a：就绪事件。token 用于定位对应 fd；interest 为实际就绪方向
// （由 poll 返回的 revents 解析；POLLERR=8/POLLHUP=16 无关注位时按可读报告，
// 连接关闭场景 recv 返回 0 由应用自行判断）。
struct Event {
    token: i64,
    interest: io::nio::Interest,
}

impl Event {
    fn is_readable(&self) -> bool {
        match self.interest {
            io::nio::Interest::Readable => true,
            io::nio::Interest::Writable => false,
            io::nio::Interest::ReadableWritable => true,
        }
    }
    fn is_writable(&self) -> bool {
        match self.interest {
            io::nio::Interest::Readable => false,
            io::nio::Interest::Writable => true,
            io::nio::Interest::ReadableWritable => true,
        }
    }
}

// R1b：事件轮询器（poll(2) 封装）。
// 内部维护注册表：fd 列表 + 关注标志（POLLIN=1 / POLLOUT=4）+ 应用侧 token。
struct Poller {
    fds: Vec<i64>,
    events: Vec<i64>,
    tokens: Vec<i64>,
}

impl Poller {
    // 创建轮询器（poll(2) 无显式创建，Poller 仅为注册表状态容器）。
    fn new() -> Result<io::nio::Poller, io::error::IoError> {
        Result::Ok(io::nio::Poller {
            fds: Vec::new(),
            events: Vec::new(),
            tokens: Vec::new(),
        })
    }
    // 注册 fd 并绑定应用侧 token；重复注册报 AlreadyExists。
    fn register(&mut self, fd: i64, token: i64, interest: io::nio::Interest) -> Result<i64, io::error::IoError> {
        let n = self.fds.len();
        let mut i = 0;
        while i < n {
            if self.fds[i] == fd {
                return Result::Err(IoError::new(
                    io::error::IoErrorKind::AlreadyExists,
                    String::from("fd already registered"),
                ));
            }
            i = i + 1;
        }
        self.fds.push(fd);
        self.events.push(io::nio::interest_events(interest));
        self.tokens.push(token);
        Result::Ok(1)
    }
    // 修改 fd 的关注事件与 token；未注册则追加注册（与 register 等价）。
    fn reregister(&mut self, fd: i64, token: i64, interest: io::nio::Interest) -> Result<i64, io::error::IoError> {
        let n = self.fds.len();
        let mut i = 0;
        while i < n {
            if self.fds[i] == fd {
                self.events[i] = io::nio::interest_events(interest);
                self.tokens[i] = token;
                return Result::Ok(1);
            }
            i = i + 1;
        }
        self.fds.push(fd);
        self.events.push(io::nio::interest_events(interest));
        self.tokens.push(token);
        Result::Ok(1)
    }
    // 注销 fd（最后一项覆盖被删项后 pop，保持注册表紧凑）。
    fn deregister(&mut self, fd: i64) -> Result<i64, io::error::IoError> {
        let n = self.fds.len();
        let mut i = 0;
        while i < n {
            if self.fds[i] == fd {
                self.fds[i] = self.fds[n - 1];
                self.events[i] = self.events[n - 1];
                self.tokens[i] = self.tokens[n - 1];
                self.fds.pop();
                self.events.pop();
                self.tokens.pop();
                return Result::Ok(1);
            }
            i = i + 1;
        }
        Result::Err(IoError::new(
            io::error::IoErrorKind::NotFound,
            String::from("fd not registered"),
        ))
    }
    // 阻塞等待就绪事件；timeout_ms < 0 表示无限等待。
    // 返回本次就绪的事件列表（每次调用重新扫描 revents，interest 为实际就绪方向）。
    fn poll(&self, timeout_ms: i64) -> Result<Vec<io::nio::Event>, io::error::IoError> {
        if __rlyeh_target_os() == 5 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("poll disabled on WASI"),
            ));
        }
        let n = self.fds.len();
        if n == 0 {
            return Result::Ok(Vec::new());
        }
        // 构造 pollfd 缓冲：8 字节/项（fd int32 小端 + events int16 小端 + revents int16 = 0）
        let mut buf = String::with_capacity(n * 8);
        let mut i = 0;
        while i < n {
            let fd = self.fds[i];
            buf.push_byte(fd & 0xFF);
            buf.push_byte((fd >> 8) & 0xFF);
            buf.push_byte((fd >> 16) & 0xFF);
            buf.push_byte((fd >> 24) & 0xFF);
            let ev = self.events[i];
            buf.push_byte(ev & 0xFF);
            buf.push_byte((ev >> 8) & 0xFF);
            buf.push_byte(0);
            buf.push_byte(0);
            i = i + 1;
        }
        let r = poll(buf, n, timeout_ms);
        if r < 0 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("poll failed"),
            ));
        }
        if r == 0 {
            return Result::Ok(Vec::new());
        }
        let mut evs_out = Vec::new();
        let mut k = 0;
        while k < n {
            let b0 = buf.data[k * 8 + 6];
            let b1 = buf.data[k * 8 + 7];
            let revents = b0 | (b1 << 8);
            if revents != 0 {
                evs_out.push(io::nio::Event {
                    token: self.tokens[k],
                    interest: io::nio::revents_interest(revents),
                });
            }
            k = k + 1;
        }
        Result::Ok(evs_out)
    }
}

// Interest → pollfd events 掩码（POLLIN=1, POLLOUT=4）。
fn interest_events(i: io::nio::Interest) -> i64 {
    match i {
        io::nio::Interest::Readable => 1,
        io::nio::Interest::Writable => 4,
        io::nio::Interest::ReadableWritable => 5,
    }
}

// revents → Interest（POLLERR/POLLHUP 无关注位时按可读报告，连接关闭可见）。
fn revents_interest(rev: i64) -> io::nio::Interest {
    if (rev & 4) != 0 {
        if (rev & 1) != 0 {
            io::nio::Interest::ReadableWritable
        } else {
            io::nio::Interest::Writable
        }
    } else {
        io::nio::Interest::Readable
    }
}

// R2：设置 fd 是否为非阻塞模式（fcntl F_GETFL=3 / F_SETFL=4；O_NONBLOCK=0x4）。
// WASI 下无 fcntl：返回 Err（禁用文档化，L4a）。
fn set_nonblocking(fd: i64, nonblocking: bool) -> Result<i64, io::error::IoError> {
    if __rlyeh_target_os() == 5 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::Other,
            String::from("fcntl disabled on WASI"),
        ));
    }
    let flags = fcntl(fd, 3, 0);
    if flags < 0 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::Other,
            String::from("fcntl F_GETFL failed"),
        ));
    }
    if nonblocking {
        if (flags & 4) != 0 {
            return Result::Ok(1);
        }
        let nf = flags | 4;
        let r = fcntl(fd, 4, nf);
        if r < 0 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("fcntl F_SETFL failed"),
            ));
        }
        Result::Ok(1)
    } else {
        if (flags & 4) == 0 {
            return Result::Ok(1);
        }
        let nf = flags - (flags & 4);   // 清 O_NONBLOCK 位（~ 未支持，减法等价）
        let r = fcntl(fd, 4, nf);
        if r < 0 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("fcntl F_SETFL failed"),
            ));
        }
        Result::Ok(1)
    }
}

// R2：查询 fd 是否处于非阻塞模式。
// WASI 下无 fcntl：返回 Err（禁用文档化，L4a）。
fn is_nonblocking(fd: i64) -> Result<bool, io::error::IoError> {
    if __rlyeh_target_os() == 5 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::Other,
            String::from("fcntl disabled on WASI"),
        ));
    }
    let flags = fcntl(fd, 3, 0);
    if flags < 0 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::Other,
            String::from("fcntl F_GETFL failed"),
        ));
    }
    Result::Ok((flags & 4) != 0)
}

// ===== Y2a（2026-08-28）：kqueue/kevent 平台后端 FFI 绑定 =====
// macOS/BSD 高性能事件后端（替代 poll 的 O(n) 扫描）。kevent 结构体 32 字节：
//   ident(uintptr 8B) + filter(int16 2B) + flags(uint16 2B) +
//   fflags(uint32 4B) + data(intptr 8B) + udata(ptr 8B)
// 常量（macOS/BSD）：EVFILT_READ=-1、EVFILT_WRITE=-2；EV_ADD=0x1、EV_DELETE=0x2。
// 仅 FFI 绑定 + 结构体构造（Y2a）；接入 Poller 分派为 Y2b。

// 构造 kevent 结构体缓冲（小端字节填充；fd 为 ident，filter/EVFILT 常量，flags/EV_* 常量）。
fn kevent_make(fd: i64, filter: i64, flags: i64) -> String {
    let mut buf = String::with_capacity(32);
    // ident: uintptr_t（8 字节小端）
    let mut i = 0;
    while i < 8 {
        buf.push_byte((fd >> (i * 8)) & 0xFF);
        i = i + 1;
    }
    // filter: int16（EVFILT_READ=-1 / EVFILT_WRITE=-2，负数经算术右移 & 0xFF）
    buf.push_byte(filter & 0xFF);
    buf.push_byte((filter >> 8) & 0xFF);
    // flags: uint16（EV_ADD=0x1 / EV_DELETE=0x2）
    buf.push_byte(flags & 0xFF);
    buf.push_byte((flags >> 8) & 0xFF);
    // fflags: uint32 = 0
    buf.push_byte(0);
    buf.push_byte(0);
    buf.push_byte(0);
    buf.push_byte(0);
    // data: intptr（8 字节）= 0
    buf.push_byte(0);
    buf.push_byte(0);
    buf.push_byte(0);
    buf.push_byte(0);
    buf.push_byte(0);
    buf.push_byte(0);
    buf.push_byte(0);
    buf.push_byte(0);
    // udata: void*（8 字节）= 0
    buf.push_byte(0);
    buf.push_byte(0);
    buf.push_byte(0);
    buf.push_byte(0);
    buf.push_byte(0);
    buf.push_byte(0);
    buf.push_byte(0);
    buf.push_byte(0);
    buf
}

// 创建 kqueue 实例，返回 kq fd（失败返回 -1）。
fn kqueue_new() -> i64 {
    kqueue()
}

// 向 kqueue 提交 kevent 变更列表（EV_ADD/EV_DELETE 等）。返回就绪事件数（<0 失败）。
fn kevent_ctl(kq: i64, changes: String, nchanges: i64) -> i64 {
    kevent(kq, changes, nchanges, String::from(""), 0, String::from(""))
}
