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

// R1b：事件轮询器（poll(2) 封装 + kqueue 分派，Y2b）。
// 内部维护注册表：fd 列表 + 关注标志（POLLIN=1 / POLLOUT=4）+ 应用侧 token。
// P1（2026-08-28）：`kq` 字段接入 kqueue——macOS(2)/BSD(4) 在 `new` 创建 kqueue，
// `register`/`deregister`/`poll` 走 kevent 分派（O(1)），其他平台 `kq=-1` 回退 poll(2)。
struct Poller {
    fds: Vec<i64>,
    events: Vec<i64>,
    tokens: Vec<i64>,
    kq: i64,
}

impl Poller {
    // 创建轮询器。macOS/BSD 建 kqueue（kq>0）；其他平台 kq=-1（回退 poll(2)）。
    fn new() -> Result<io::nio::Poller, io::error::IoError> {
        let kq = if __rlyeh_target_os() == 2 || __rlyeh_target_os() == 4 {
            io::nio::kqueue_new()
        } else {
            -1
        };
        Result::Ok(io::nio::Poller {
            fds: Vec::new(),
            events: Vec::new(),
            tokens: Vec::new(),
            kq: kq,
        })
    }
    // 注册 fd 并绑定应用侧 token；重复注册报 AlreadyExists。
    // macOS/BSD 额外向 kqueue 提交 EV_ADD（按 interest 映射 EVFILT_READ/WRITE）。
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
        if self.kq >= 0 {
            io::nio::kq_change(self.kq, fd, io::nio::interest_filter_mask(interest), 1);
        }
        self.fds.push(fd);
        self.events.push(io::nio::interest_events(interest));
        self.tokens.push(token);
        Result::Ok(1)
    }
    // 修改 fd 的关注事件与 token；未注册则追加注册（与 register 等价）。
    // macOS/BSD 额外向 kqueue 重新提交 EV_ADD（kevent 同 ident+filter 覆盖，删旧增新由 mask 决定）。
    fn reregister(&mut self, fd: i64, token: i64, interest: io::nio::Interest) -> Result<i64, io::error::IoError> {
        let n = self.fds.len();
        let mut i = 0;
        while i < n {
            if self.fds[i] == fd {
                if self.kq >= 0 {
                    // 删旧 filter（旧 poll 掩码→filter mask），加新 filter（新 mask）
                    io::nio::kq_change(self.kq, fd, io::nio::events_to_filter_mask(self.events[i]), 2);
                    io::nio::kq_change(self.kq, fd, io::nio::interest_filter_mask(interest), 1);
                }
                self.events[i] = io::nio::interest_events(interest);
                self.tokens[i] = token;
                return Result::Ok(1);
            }
            i = i + 1;
        }
        if self.kq >= 0 {
            io::nio::kq_change(self.kq, fd, io::nio::interest_filter_mask(interest), 1);
        }
        self.fds.push(fd);
        self.events.push(io::nio::interest_events(interest));
        self.tokens.push(token);
        Result::Ok(1)
    }
    // 注销 fd（最后一项覆盖被删项后 pop，保持注册表紧凑）。
    // macOS/BSD 额外向 kqueue 提交 EV_DELETE。
    fn deregister(&mut self, fd: i64) -> Result<i64, io::error::IoError> {
        let n = self.fds.len();
        let mut i = 0;
        while i < n {
            if self.fds[i] == fd {
                if self.kq >= 0 {
                    io::nio::kq_change(self.kq, fd, io::nio::events_to_filter_mask(self.events[i]), 2);
                }
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
    // macOS/BSD 且 kq>=0 走 kqueue（kevent_wait + 解析）；否则回退 poll(2)。
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
        if self.kq >= 0 {
            return self.kq_poll(timeout_ms);
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
    // Poller::poll 的 kqueue 分派：等待 + 解析 kevent 列表。
    // 每项 32 字节：ident(fd 低 4B) + filter(int16 u16：EVFILT_READ=-1→0xFFFF/EVFILT_WRITE=-2→0xFFFE)。
    // 用 fd 在注册表反查 token；filter 判断 interest（可读/可写）。
    fn kq_poll(&self, timeout_ms: i64) -> Result<Vec<io::nio::Event>, io::error::IoError> {
        let n = self.fds.len();
        let res = io::nio::kevent_wait_res(self.kq, n, timeout_ms);
        if res.count < 0 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("kevent failed"),
            ));
        }
        let mut evs_out = Vec::new();
        let mut k = 0;
        while k < res.count {
            let base = k * 32;
            let fd = res.evlist.data[base]
                | (res.evlist.data[base + 1] << 8)
                | (res.evlist.data[base + 2] << 16)
                | (res.evlist.data[base + 3] << 24);
            let filt = res.evlist.data[base + 8] | (res.evlist.data[base + 9] << 8);
            let mut j = 0;
            while j < n {
                if self.fds[j] == fd {
                    let interest = if filt == 65535 {
                        io::nio::Interest::Readable
                    } else {
                        io::nio::Interest::Writable
                    };
                    evs_out.push(io::nio::Event {
                        token: self.tokens[j],
                        interest: interest,
                    });
                }
                j = j + 1;
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
// `__rlyeh_kqueue` 由 driver 按目标注入（macOS/BSD 原生、其他平台 stub -1）。
fn kqueue_new() -> i64 {
    __rlyeh_kqueue()
}

// 向 kqueue 提交 kevent 变更列表（EV_ADD/EV_DELETE 等）。返回就绪事件数（<0 失败）。
fn kevent_ctl(kq: i64, changes: String, nchanges: i64) -> i64 {
    __rlyeh_kevent(kq, changes, nchanges, String::from(""), 0, String::from(""))
}

// Y2b：等待 kqueue 就绪事件（阻塞）。timeout 为毫秒（-1 无限）。
// 返回就绪 kevent 结构体缓冲（32 字节/项，data[i] 即 i 项第 i 字节）。
fn kevent_wait(kq: i64, nevents: i64, timeout_ms: i64) -> String {
    let mut evlist = String::with_capacity(nevents * 32);
    let mut i = 0;
    while i < nevents * 32 {
        evlist.push_byte(0);
        i = i + 1;
    }
    // timeout: `{tv_sec i64, tv_nsec i64}` 16 字节缓冲（String）或空（无限）。
    // MVP：timeout_ms < 0 用空（无限）；否则构造 timespec。
    let t = if timeout_ms < 0 {
        String::from("")
    } else {
        let mut tb = String::with_capacity(16);
        let sec = timeout_ms / 1000;
        let nsec = (timeout_ms % 1000) * 1000000;
        let mut j = 0;
        while j < 8 {
            tb.push_byte((sec >> (j * 8)) & 0xFF);
            j = j + 1;
        }
        j = 0;
        while j < 8 {
            tb.push_byte((nsec >> (j * 8)) & 0xFF);
            j = j + 1;
        }
        tb
    };
    let n = __rlyeh_kevent(kq, String::from(""), 0, evlist, nevents, t);
    evlist
}

// ===== P1（2026-08-28）：Poller kqueue 分派辅助 =====
// interest → filter 掩码（bit0=EVFILT_READ(-1), bit1=EVFILT_WRITE(-2)）。
fn interest_filter_mask(i: io::nio::Interest) -> i64 {
    match i {
        io::nio::Interest::Readable => 1,
        io::nio::Interest::Writable => 2,
        io::nio::Interest::ReadableWritable => 3,
    }
}
// poll 掩码（interest_events：Readable=1/Writable=4/ReadableWritable=5）→ filter 掩码。
fn events_to_filter_mask(ev: i64) -> i64 {
    if (ev & 4) != 0 {
        if (ev & 1) != 0 {
            3
        } else {
            2
        }
    } else {
        1
    }
}
// 构造 kevent changes 缓冲（含 mask 中各 filter 的一个 kevent，flags=EV_ADD(1)/EV_DELETE(2)）。
fn kevent_changes(fd: i64, mask: i64, flags: i64) -> String {
    let mut buf = String::with_capacity(64);
    if (mask & 1) != 0 {
        let ev = io::nio::kevent_make(fd, -1, flags);   // EVFILT_READ
        let mut i = 0;
        while i < 32 {
            buf.push_byte(ev.data[i]);
            i = i + 1;
        }
    }
    if (mask & 2) != 0 {
        let ev = io::nio::kevent_make(fd, -2, flags);   // EVFILT_WRITE
        let mut i = 0;
        while i < 32 {
            buf.push_byte(ev.data[i]);
            i = i + 1;
        }
    }
    buf
}
// 计算 mask 的 filter 数（nchanges）。
fn filter_count(mask: i64) -> i64 {
    let mut c = 0;
    if (mask & 1) != 0 { c = c + 1; }
    if (mask & 2) != 0 { c = c + 1; }
    c
}
// 向 kqueue 提交 EV_ADD(flags=1)/EV_DELETE(flags=2) 的 kevent 变更。
fn kq_change(kq: i64, fd: i64, mask: i64, flags: i64) -> i64 {
    let ch = io::nio::kevent_changes(fd, mask, flags);
    io::nio::kevent_ctl(kq, ch, io::nio::filter_count(mask))
}
// 就绪结果：count = 实际就绪 kevent 数，evlist = 32 字节/项缓冲。
struct KeventRes {
    count: i64,
    evlist: String,
}
// 等待 kqueue 就绪事件并返回就绪数 + 缓冲（供 Poller::poll 解析）。
fn kevent_wait_res(kq: i64, nevents: i64, timeout_ms: i64) -> io::nio::KeventRes {
    let mut evlist = String::with_capacity(nevents * 32);
    let mut i = 0;
    while i < nevents * 32 {
        evlist.push_byte(0);
        i = i + 1;
    }
    let t = if timeout_ms < 0 {
        String::from("")
    } else {
        let mut tb = String::with_capacity(16);
        let sec = timeout_ms / 1000;
        let nsec = (timeout_ms % 1000) * 1000000;
        let mut j = 0;
        while j < 8 {
            tb.push_byte((sec >> (j * 8)) & 0xFF);
            j = j + 1;
        }
        j = 0;
        while j < 8 {
            tb.push_byte((nsec >> (j * 8)) & 0xFF);
            j = j + 1;
        }
        tb
    };
    let cnt = __rlyeh_kevent(kq, String::from(""), 0, evlist, nevents, t);
    io::nio::KeventRes { count: cnt, evlist: evlist }
}

