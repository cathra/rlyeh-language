// Y2a：kqueue/kevent FFI 绑定（io/nio.rl）
// kqueue() 创建 + kevent_make 构造 32 字节结构体 + kevent_ctl 提交（EV_ADD）。
// 用真实 socketpair fd 验证注册（EVFILT_READ）。macOS/BSD 平台。

fn main() {
    // 1. kqueue() 创建
    let kq = io::nio::kqueue_new();
    // 2. 创建 socketpair，取 fd[0] 作为可读监视对象
    let sp = net::socketpair_stream();
    let fd = net::fd_at(sp, 0);
    // 3. 构造 kevent（fd, EVFILT_READ=-1, EV_ADD=0x1）验证结构体字节布局
    let ev = io::nio::kevent_make(fd, -1, 1);
    let b0 = ev.data[0];
    let b8 = ev.data[8];
    let b10 = ev.data[10];
    // 4. kevent() 提交 EV_ADD（无就绪事件，返回 0；失败 <0）
    let r = io::nio::kevent_ctl(kq, ev, 1);

    let mut total = 0;
    if b0 == fd {
        total = total + 1;
    }
    if b8 == 255 {
        total = total + 1;
    }
    if b10 == 1 {
        total = total + 1;
    }
    if kq >= 0 {
        total = total + 1;
    }
    if r >= 0 {
        total = total + 1;
    }
    println(total); // 5
}
