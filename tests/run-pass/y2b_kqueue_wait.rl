// Y2b：kqueue/kevent 等待真实就绪事件（io/nio.rl 平台后端核心）
// kqueue() + kevent_make(EVFILT_READ) + kevent_ctl(EV_ADD) + kevent_wait。
// 用 socketpair：fd1 写数据 → fd0 EVFILT_READ 就绪，kevent 等待返回 ident=fd0。
// macOS/BSD 平台（WASI 下 kqueue 不可用，Poller 分派时走 poll 兜底）。

fn main() {
    // 1. 创建 kqueue + socketpair
    let kq = io::nio::kqueue_new();
    let sp = net::socketpair_stream();
    let fd0 = net::fd_at(sp, 0);
    let fd1 = net::fd_at(sp, 1);
    // 2. 注册 fd0 EVFILT_READ（EV_ADD）
    let ev = io::nio::kevent_make(fd0, -1, 1); // EVFILT_READ=-1, EV_ADD=1
    let r_add = io::nio::kevent_ctl(kq, ev, 1);
    // 3. 向 fd1 写数据，使 fd0 可读
    let w = net::send_all(fd1, String::from("x"));
    let w_ok = match w {
        Result::Ok(_) => 1,
        Result::Err(_) => 0,
    };
    // 4. kevent 等待就绪（100ms）
    let evlist = io::nio::kevent_wait(kq, 4, 100);
    // 5. 解析首项就绪事件 ident（uintptr 8 字节小端前 4 字节）
    let ident = evlist.data[0]
        | (evlist.data[1] << 8)
        | (evlist.data[2] << 16)
        | (evlist.data[3] << 24);

    let mut total = 0;
    if kq >= 0 {
        total = total + 1;
    }
    if r_add >= 0 {
        total = total + 1;
    }
    if w_ok == 1 {
        total = total + 1;
    }
    if ident == fd0 {
        total = total + 1;
    }
    println(total); // 4
}
