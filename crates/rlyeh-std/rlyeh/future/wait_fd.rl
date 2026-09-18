// future/wait_fd.rl：fd 事件 future（`WaitFd` / `wait_fd`）——2026-09-18 由 future/module.rl 拆出。
//
// 归属子模块 `future::wait_fd`。对外 `future::WaitFd` / `future::wait_fd` 由
// future/module.rl 的 `pub import` 保持。

// W3 第二步（2026-08-25）：等待某 fd 的关注事件就绪。poll 时经 `Poller::poll(0)`
// 非阻塞检查；未就绪则向 `cx.fd` / `cx.interest`（掩码：POLLIN=1 / POLLOUT=4 /
// 读写=5）请求唤醒，Pending；就绪返回 `Ready(0)`。供事件驱动 executor 经 poll(2)
// 等 fd 就绪（替代忙等）。
struct WaitFd {
    fd: i64,
    interest: i64,
}

fn wait_fd(fd: i64, interest: io::nio::Interest) -> future::wait_fd::WaitFd {
    WaitFd {
        fd: fd,
        interest: io::nio::interest_events(interest),
    }
}

impl WaitFd: Future {
    type Output = i64;
    fn poll(&mut self, cx: &mut future::poll::Context) -> future::poll::Poll<Self::Output> {
        match Poller::new() {
            Result::Ok(p) => {
                let mut q = p;
                // 关注掩码 → Interest 枚举（POLLIN=1 / POLLOUT=4 / 读写=5）
                let int_enum = if (self.interest & 4) != 0 {
                    if (self.interest & 1) != 0 {
                        io::nio::Interest::ReadableWritable
                    } else {
                        io::nio::Interest::Writable
                    }
                } else {
                    io::nio::Interest::Readable
                };
                match q.register(self.fd, 0, int_enum) {
                    Result::Ok(_) => {
                        match q.poll(0) {
                            Result::Ok(evs) => {
                                if evs.len() > 0 {
                                    future::poll::Poll::Ready(0)
                                } else {
                                    cx.fd = self.fd;
                                    cx.interest = self.interest;
                                    future::poll::Poll::Pending
                                }
                            }
                            Result::Err(_) => future::poll::Poll::Ready(-1),
                        }
                    }
                    Result::Err(_) => future::poll::Poll::Ready(-2),
                }
            }
            Result::Err(_) => future::poll::Poll::Ready(-3),
        }
    }
}
