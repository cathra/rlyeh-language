// sync/channel.rl：无界/有界并发通道 + 异步接收 future（P1/P7c/W5，2026-08）
// ===== Channel（P1，2026-08）：无界并发队列 =====
// P7c（2026-08-29）：元素类型参数化 `Channel<T>`（此前 MVP 限 i64）。
// 队列状态经 Arc<Channel<T>> 共享（Sender/Receiver 各自持有 clone；原子引用计数，可跨线程安全共享）。
// recv 空队列挂起（Condvar wait），send 后 notify_one 唤醒；
// close 后队列耗尽 recv 返回 None（try_recv 空返回 None，不阻塞）。
// 注意：queue 只增（head 单调推进，无元素移除的 MVP 简化）。
// W5（2026-08-25）：`wake_r`/`wake_w` 为 socketpair 唤醒 fd——`send`/`close`
// 向 `wake_w` 写字节，`recv_async` 的 future 经 `wake_r` 读就绪挂起（W3
// `wait_fd`/`future::poll::Context.fd` 事件驱动，非阻塞线程），实现「挂起直到数据/close」。
struct Channel<T> {
    m: sync::Mutex<i64>,
    cv: sync::Condvar,
    closed: i64,
    head: i64,
    queue: Vec<T>,
    wake_r: i64,
    wake_w: i64,
    capacity: i64,   // 0 = 无界；>0 = 有界容量（满则 send 阻塞）
}

struct Sender<T> { ch: Arc<sync::Channel<T>> }
struct Receiver<T> { ch: Arc<sync::Channel<T>> }
// 元组返回类型 MVP 未实现（(1, 2) 被解析为集合），channel() 返回结构体对。
struct ChannelPair<T> { tx: sync::Sender<T>, rx: sync::Receiver<T> }

// Y4c（2026-08-30）：通道错误类型——`Result` 语义（替代 MVP `Option` 退化）。
// `recv`/`try_recv`/`send` 既有的 `Option`/`()` 便捷 API 保留以兼容既有用例；
// 错误类型经 `recv_result`/`try_recv_result`/`send_result` 暴露。
struct SendError<T> { val: T }             // send 失败：通道已关闭（无接收方）
struct RecvError { disconnected: i64 }     // recv 失败：通道已关闭且队列空
struct TryRecvError { kind: i64 }          // 0 = 空（Empty），1 = 已断开（Disconnected）

// W5（2026-08-25）：异步接收 future（`Receiver::recv_async` 返回值）。
// poll：先消费唤醒字节（避免 fd 永久就绪忙等），try_recv 非阻塞取消息——
// 有则 `Ready(Some(v))`；空且未关闭则向 `cx.fd`（`wake_r` 读）注册挂起，由事件驱动
// executor（W3 `block_on`）经 poll(2) 等 `send`/`close` 写的唤醒字节就绪再轮询。
// P7c：`Output = Option<T>`——close 且空返回 `None`（替代 MVP 哨兵 `-1`，
// 哨兵仅对 i64 有效，泛型化后改用 Option 表达「关闭且空」）。
struct RecvAsync<T> {
    ch: Arc<sync::Channel<T>>,
}

impl<T> RecvAsync<T>: Future {
    type Output = Option<T>;
    fn poll(&mut self, cx: &mut future::poll::Context) -> future::poll::Poll<Self::Output> {
        // 消费唤醒字节（send/close 写入），避免 fd 永久就绪导致忙等
        let _ = net::recv_some(self.ch.wake_r, 64);
        // 非阻塞取消息
        let mut r = sync::Receiver<T> { ch: self.ch.clone() };
        match r.try_recv() {
            Option::Some(v) => future::poll::Poll::Ready(Option::Some(v)),
            Option::None => {
                if self.ch.closed != 0 {
                    future::poll::Poll::Ready(Option::None)
                } else {
                    cx.fd = self.ch.wake_r;
                    cx.interest = 1;
                    future::poll::Poll::Pending
                }
            }
        }
    }
}

// P7c：泛型通道构造——调用点用 turbofish 指定元素类型（`channel::<i64>()`）。
fn channel<T>() -> sync::ChannelPair<T> {
    // 构造调用用裸名（`sync::Mutex::new()` 路径 typecheck 不支持；
    // 裸名经 core.rl `import sync::Mutex` 别名解析为 sync::Mutex）
    // W5：创建 socketpair 唤醒 fd（`send`/`close` 写 `wake_w` 触发 `wake_r` 读就绪）
    let sp = net::socketpair_stream();
    let wake_r = net::fd_at(sp, 0);
    let wake_w = net::fd_at(sp, 1);
    // P7c：构造显式带泛型实参（`sync::Channel<T>` / `Sender<T>` / `Receiver<T>`）——
    // Arc 嵌套（`Arc<Channel<T>>`）时字段推断无法反推外层 T，须显式给出。
    let ch = Arc::new(sync::Channel<T> {
        m: Mutex::new(0),
        cv: Condvar::new(),
        closed: 0,
        head: 0,
        queue: Vec::with_capacity(8),
        wake_r: wake_r,
        wake_w: wake_w,
        capacity: 0,   // 无界
    });
    sync::ChannelPair<T> {
        tx: sync::Sender<T> { ch: ch.clone() },
        rx: sync::Receiver<T> { ch: ch },
    }
}

// Y4c（2026-08-30）：有界通道构造——`capacity > 0` 限制在途元素数，
// 满则 `send` 经 condvar 挂起阻塞（消费者腾出空间后唤醒），提供背压。
fn bounded_channel<T>(capacity: i64) -> sync::ChannelPair<T> {
    let sp = net::socketpair_stream();
    let wake_r = net::fd_at(sp, 0);
    let wake_w = net::fd_at(sp, 1);
    let ch = Arc::new(sync::Channel<T> {
        m: Mutex::new(0),
        cv: Condvar::new(),
        closed: 0,
        head: 0,
        queue: Vec::with_capacity(8),
        wake_r: wake_r,
        wake_w: wake_w,
        capacity: capacity,
    });
    sync::ChannelPair<T> {
        tx: sync::Sender<T> { ch: ch.clone() },
        rx: sync::Receiver<T> { ch: ch },
    }
}

impl<T> Sender<T> {
    // 发送（无界队列永不阻塞；唤醒一个等待中的接收者）。
    // `r#` 转义：`send` 为 actor 保留字（定义名归一化为 send，调用处 `.send(...)` 可用）
    // W5：向 `wake_w` 写唤醒字节，使 `recv_async` 挂起的 fd 读就绪（事件驱动）。
    // P7c：元素类型参数化（`val: T`）
    fn r#send(&mut self, val: T) {
        self.ch.m.lock();
        // 有界：队列满（len - head >= capacity）则挂起等待消费者腾出空间
        while self.ch.capacity > 0 && (self.ch.queue.len() - self.ch.head) >= self.ch.capacity {
            self.ch.cv.wait(self.ch.m);
        }
        self.ch.queue.push(val);
        self.ch.cv.notify_one();
        let _ = net::send_all(self.ch.wake_w, String::from("x"));
        self.ch.m.unlock();
    }
    // 尝试发送：有界队列满则返回 false；无界恒成功
    fn try_send(&mut self, val: T) -> bool {
        self.ch.m.lock();
        if self.ch.capacity > 0 && (self.ch.queue.len() - self.ch.head) >= self.ch.capacity {
            self.ch.m.unlock();
            return false;
        }
        self.ch.queue.push(val);
        self.ch.cv.notify_one();
        let _ = net::send_all(self.ch.wake_w, String::from("x"));
        self.ch.m.unlock();
        true
    }
    // send_result：Result 语义发送——通道已关闭返回 Err(SendError{val})，否则阻塞发送成功
    fn send_result(&mut self, val: T) -> Result<i64, sync::SendError<T>> {
        if self.ch.closed != 0 {
            return Result::Err(sync::SendError<T> { val: val });
        }
        self.r#send(val);
        Result::Ok(0)
    }
    // 关闭通道：广播唤醒等待者，队列耗尽后所有 Receiver recv 返回 None
    // W5：写唤醒字节，使 recv_async 挂起被唤醒（读到 close 标记）。
    fn close(&mut self) {
        self.ch.m.lock();
        self.ch.closed = 1;
        self.ch.cv.notify_all();
        let _ = net::send_all(self.ch.wake_w, String::from("x"));
        self.ch.m.unlock();
    }
    // 多 Sender 共享同一队列（Arc clone）
    fn clone(self) -> sync::Sender<T> {
        sync::Sender { ch: self.ch.clone() }
    }
}

impl<T> Receiver<T> {
    // 阻塞接收：队列空且未关闭时挂起等待；关闭且空返回 None
    fn r#recv(&mut self) -> Option<T> {
        loop {
            self.ch.m.lock();
            if self.ch.queue.len() > self.ch.head {
                let v = self.ch.queue[self.ch.head];
                self.ch.head += 1;
                self.ch.cv.notify_one();   // 唤醒阻塞中的发送者（有界队列满）
                self.ch.m.unlock();
                return Option::Some(v);
            }
            if self.ch.closed != 0 {
                self.ch.m.unlock();
                return Option::None;
            }
            self.ch.cv.wait(self.ch.m);
            self.ch.m.unlock();
        }
    }
    // 尝试接收：非空立即返回 Some(v)，空返回 None（不阻塞）
    fn try_recv(&mut self) -> Option<T> {
        self.ch.m.lock();
        if self.ch.queue.len() > self.ch.head {
            let v = self.ch.queue[self.ch.head];
            self.ch.head += 1;
            self.ch.cv.notify_one();   // 唤醒阻塞中的发送者（有界队列满）
            self.ch.m.unlock();
            return Option::Some(v);
        }
        self.ch.m.unlock();
        Option::None
    }
    // recv_result：阻塞接收 Result 语义——关闭且空返回 Err(RecvError)
    fn recv_result(&mut self) -> Result<T, sync::RecvError> {
        match self.r#recv() {
            Option::Some(v) => Result::Ok(v),
            Option::None => Result::Err(sync::RecvError { disconnected: 1 }),
        }
    }
    // try_recv_result：非阻塞 Result 语义——空 Err(kind:0)，断开 Err(kind:1)
    fn try_recv_result(&mut self) -> Result<T, sync::TryRecvError> {
        self.ch.m.lock();
        if self.ch.queue.len() > self.ch.head {
            let v = self.ch.queue[self.ch.head];
            self.ch.head += 1;
            self.ch.cv.notify_one();   // 唤醒阻塞中的发送者（有界队列满）
            self.ch.m.unlock();
            Result::Ok(v)
        } else if self.ch.closed != 0 {
            self.ch.m.unlock();
            Result::Err(sync::TryRecvError { kind: 1 })
        } else {
            self.ch.m.unlock();
            Result::Err(sync::TryRecvError { kind: 0 })
        }
    }
    // S3a/W5：异步接收——返回 `RecvAsync<T>` future，`async fn` 内经
    // `let r: sync::RecvAsync<T> = rx.recv_async(); r.await` 挂起（不阻塞线程）。
    // future 的 poll：try_recv 非阻塞取消息（有则 `Ready(Some(v))`），空且未关闭则向
    // `cx.fd`（wake_r）注册读就绪挂起；`send`/`close` 写唤醒字节触发 poll 重查。
    // P7c：`Output = Option<T>`（close 且空 → `Ready(None)`，替代 MVP 哨兵 `-1`）。
    fn recv_async(&mut self) -> sync::RecvAsync<T> {
        sync::RecvAsync<T> { ch: self.ch.clone() }
    }
    // J2 迭代器接入：for v in rx { ... }（内部 try_recv 语义，不阻塞）
    fn next(&mut self) -> Option<T> {
        self.try_recv()
    }
    // 多 Receiver 共享同一队列（Arc clone）
    fn clone(self) -> sync::Receiver<T> {
        sync::Receiver { ch: self.ch.clone() }
    }
}
