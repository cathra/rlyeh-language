// ===== future 模块（S1/S2，2026-08；W1 泛型化 + W4 TimeoutError 2026-08-25）：异步运行时基础 =====
// S1a：`Poll` 枚举 + `Future` trait；S1b：`block_on` 手动轮询；
// S2c：`timeout` 带超时轮询。
// 实现说明（与 std-lib.md §10 规划 API 的 MVP 退化对照）：
// - W1 ✅：关联类型 `type Output` 已可用（U2），`Future::poll` 签名对齐规划——
//   `fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output>`；`Context` 为
//   占位类型（规划 `Context<'_>` + `Pin<&mut Self>`；`Pin` 语义 MVP 退化
//   `&mut self` 聚合指针，`Context` 保留参数位、无唤醒方法）。
// - 泛型约束 `F: Future` 仍不可用——`block_on` 采用泛型函数形态
//   `block_on<T>(f: &mut T)`，实例化时按具体类型解析 `poll`（宽松语义）。
// - 手动轮询：`loop { match f.poll(&mut cx) { Ready(v) => return v, Pending => .. } }`。
// - S1c（async/await 状态机 desugar）与 Future 版 `join_all`（S2）后续实现。

// S1a：轮询结果（泛型枚举，与 Option 同构）。
enum Poll<T> {
    Ready(T),
    Pending,
}

// W1/W3：Context（poll 上下文）。W1 ✅ 保留签名参数位；W3（2026-08-25）升级
// 携带唤醒请求槽——future 在 `Pending` 时可写入「何时/何事件应被重新 poll」，
// 供事件驱动 executor（`block_on`/`timeout`）据此休眠或等事件再轮询（替代忙等）：
// - `deadline`：定时器唤醒截止（单调时钟微秒，`__rlyeh_clock_monotonic`）；
// - `fd` / `interest`：fd 事件唤醒（W3 第二步）——`fd` 为要监听的 fd（0 = 无），
//   `interest` 为关注方向掩码（1 = POLLIN 读 / 4 = POLLOUT 写）。
// 全 0 表示未请求（executor 退回忙等，向后兼容）。规划 `Context<'a>`（Waker
// 引用 + 唤醒器 API）语义 MVP 退化；结构体须非空（`_unit` 哨兵字段保留）。
struct Context {
    _unit: i64,
    deadline: i64,
    fd: i64,
    interest: i64,
}

// S1a/W1：Future trait——`poll` 推进状态机，返回 `Ready(值)` 或 `Pending`。
// 关联类型 `type Output` 声明输出类型（U2 ✅）；`&mut self` 聚合指针传递。
trait Future {
    type Output;
    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output>;
}

// S1b/W3：阻塞轮询 `f` 直到 `poll` 返回 `Ready`，返回其携带值（`F::Output`，
// W4 关联类型投影——支持任意输出类型，不再限 i64）。
// 调用方把状态机实现为 `Future` 类型后 `block_on(&mut fut)`；泛型实参
// 由实参类型推断（MVP 实例化时重查 body，`f.poll()` 解析到具体 impl）。
// W3（2026-08-25）事件驱动：`Pending` 且 future 已通过 `cx.deadline` 请求
// 下次唤醒时刻时，休眠到该时刻再轮询（不忙等）；`deadline = 0` 退回忙等
// （向后兼容不依赖定时器的 future）。
fn block_on<F: Future>(f: &mut F) -> F::Output {
    let mut cx = Context { _unit: 0, deadline: 0, fd: 0, interest: 0 };
    loop {
        match f.poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => {
                let d = cx.deadline;
                let fd = cx.fd;
                let interest = cx.interest;
                cx.deadline = 0;
                cx.fd = 0;
                cx.interest = 0;
                if fd > 0 {
                    // fd 事件唤醒：poll 等 fd 就绪（timeout = deadline 剩余 ms 或 -1 无限）
                    let timeout_ms = if d <= 0 {
                        -1
                    } else {
                        let now = __rlyeh_clock_monotonic();
                        let cur = if now >= 0 { now } else { clock() };
                        if d > cur { (d - cur) / 1000 } else { 0 }
                    };
                    match Poller::new() {
                        Result::Ok(p) => {
                            let mut q = p;
                            let int_enum = if (interest & 4) != 0 {
                                if (interest & 1) != 0 {
                                    io::nio::Interest::ReadableWritable
                                } else {
                                    io::nio::Interest::Writable
                                }
                            } else {
                                io::nio::Interest::Readable
                            };
                            match q.register(fd, 0, int_enum) {
                                Result::Ok(_) => {
                                    let _ = q.poll(timeout_ms);
                                }
                                Result::Err(_) => {}
                            }
                        }
                        Result::Err(_) => {}
                    }
                } else if d > 0 {
                    let now = __rlyeh_clock_monotonic();
                    let cur = if now >= 0 { now } else { clock() };
                    if d > cur {
                        thread::sleep(Duration::microseconds(d - cur));
                    }
                }
            }
        }
    }
}

// W3（2026-08-25）：定时器 future——`future::sleep(duration)` 在 `duration`
// 内 `Pending`（向 `cx.deadline` 请求唤醒时刻），到时 `Ready(0)`。供事件驱动
// executor 休眠而非忙等；经 `&mut *cx` 透传写 deadline（与子 future 共享同一
// Context 实例，外层/executor 可读）。
struct Sleep {
    target: i64,
}

fn sleep(duration: time::Duration) -> future::Sleep {
    let d = duration.micros();
    let t0 = __rlyeh_clock_monotonic();
    let start = if t0 >= 0 { t0 } else { clock() };
    future::Sleep { target: start + d }
}

impl Future for Sleep {
    type Output = i64;
    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output> {
        let t0 = __rlyeh_clock_monotonic();
        let now = if t0 >= 0 { t0 } else { clock() };
        if now >= self.target {
            Poll::Ready(0)
        } else {
            cx.deadline = self.target;
            Poll::Pending
        }
    }
}

// W3 第二步（2026-08-25）：fd 事件 future——`future::wait_fd(fd, interest)`
// 等待某 fd 的关注事件就绪。poll 时经 `Poller::poll(0)` 非阻塞检查；未就绪则
// 向 `cx.fd`/`cx.interest`（掩码：POLLIN=1/POLLOUT=4/读写=5）请求唤醒，Pending；
// 就绪返回 `Ready(0)`。供事件驱动 executor 经 poll(2) 等 fd 就绪（替代忙等）。
struct WaitFd {
    fd: i64,
    interest: i64,
}

fn wait_fd(fd: i64, interest: io::nio::Interest) -> future::WaitFd {
    future::WaitFd {
        fd: fd,
        interest: io::nio::interest_events(interest),
    }
}

impl Future for WaitFd {
    type Output = i64;
    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output> {
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
                                    Poll::Ready(0)
                                } else {
                                    cx.fd = self.fd;
                                    cx.interest = self.interest;
                                    Poll::Pending
                                }
                            }
                            Result::Err(_) => Poll::Ready(-1),
                        }
                    }
                    Result::Err(_) => Poll::Ready(-2),
                }
            }
            Result::Err(_) => Poll::Ready(-3),
        }
    }
}

// W4（2026-08-25）：Future 超时错误类型（std-lib.md §12 错误体系扩展）。
// 参照 `IoError` 模式：消息字符串 + `Error::message` 访问器。完整 Display
// trait 随 X4（Formatter 完整化）落地，MVP 用 `message()`。
struct TimeoutError {
    message: String,
}

impl TimeoutError {
    // MVP：无载荷超时错误，消息固定；正式版可携带 `Duration` 等上下文。
    fn new() -> TimeoutError {
        TimeoutError {
            message: String::from("future timed out"),
        }
    }
    fn message(&self) -> String {
        self.message
    }
}

impl Error for TimeoutError {
    fn message(&self) -> String {
        self.message
    }
}

// S2c/W4：带超时阻塞轮询——`duration` 内未 `Ready` 返回 `Err(TimeoutError)`（超时）。
// 对齐 std-lib §10 规划 API 参数顺序（duration 在前）与输出 `Result<F::Output, TimeoutError>`
// ——`F::Output` 关联类型投影（泛型参数上的关联类型）落地，输出不再限 `i64`。
// - 泛型 `f: &mut F` 带 `F: Future` 约束（实例化时按具体类型解析 poll，
//   返回类型 `Result<F::Output, TimeoutError>`）；
// - 超时判定经墙钟 `__rlyeh_clock_monotonic`（S2b ✅ clock_gettime
//   MONOTONIC，与 `time::Instant` 相同的退化逻辑：返回 -1 退回 `clock()`
//   CPU 时钟）。注：MVP 静态方法调用不支持模块路径前缀
//   （`time::Instant::now` 不可用），故直接复用 extern；
// - W3（2026-08-25）事件驱动：`Pending` 且 future 经 `cx.deadline` 请求唤醒
//   时刻时，休眠到该时刻或超时截止（取更早），替代忙等；`deadline = 0`
//   退回忙等（向后兼容）。
fn timeout<F: Future>(duration: time::Duration, f: &mut F) -> Result<F::Output, TimeoutError> {
    let limit = duration.micros();
    let t0 = __rlyeh_clock_monotonic();
    let start = if t0 >= 0 { t0 } else { clock() };
    let mut cx = Context { _unit: 0, deadline: 0, fd: 0, interest: 0 };
    loop {
        match f.poll(&mut cx) {
            Poll::Ready(v) => return Result::Ok(v),
            Poll::Pending => {
                let d = cx.deadline;
                cx.deadline = 0;
                cx.fd = 0;
                cx.interest = 0;
                let t1 = __rlyeh_clock_monotonic();
                let cur = if t1 >= 0 { t1 } else { clock() };
                if cur - start >= limit {
                    return Result::Err(TimeoutError::new());
                }
                // W3：事件驱动休眠——future 经 cx.deadline 请求唤醒时刻时，sleep
                // 到该时刻或超时截止（取更早），避免忙等。
                if d > 0 {
                    let limit_at = start + limit;
                    let wait_until = if d < limit_at { d } else { limit_at };
                    if wait_until > cur {
                        thread::sleep(Duration::microseconds(wait_until - cur));
                    }
                }
            }
        }
    }
}

// W4（2026-08-25）补全：并发轮询多个 Future 直至全部 `Ready`，按传入顺序收集输出。
// 对齐 std-lib.md §10 规划 `join_all<F: Future>(Vec<F>) -> Vec<F::Output>`——
// `F::Output` 关联类型投影（泛型参数上的关联类型）落地，输出不再限 `i64`。
// - 并发语义：反复轮询所有未完成的 future（busy-wait），每轮推进其状态
//   （`futures[i]` 取出 → `poll` → 写回持久），直至全部 `Ready`；
// - `done` 数组标记已完成槽位，避免重复轮询已就绪的 future；
// - 事件驱动等待规划随 W3（executor），MVP 为忙等轮询。
fn join_all<F: Future>(futures: Vec<F>) -> Vec<F::Output> {
    let n = futures.len();
    // 完成标记（0=未完成，1=已完成）
    let mut done: Vec<i64> = Vec::with_capacity(n);
    let mut i = 0;
    while i < n {
        done.push(0);
        i = i + 1;
    }
    // 结果收集：index → Output（HashMap 支持任意 F::Output，无需 i64 占位预填充）
    let mut results: HashMap<i64, F::Output> = HashMap::new();
    let mut finished = 0;
    let mut futures = futures;
    while finished < n {
        let mut cx = Context { _unit: 0, deadline: 0, fd: 0, interest: 0 };
        let mut it = 0;
        while it < n {
            if done[it] == 0 {
                let mut cur = futures[it];
                match cur.poll(&mut cx) {
                    Poll::Ready(v) => {
                        results.insert(it, v);
                        finished = finished + 1;
                        futures[it] = cur;
                        done[it] = 1;
                    }
                    Poll::Pending => {
                        futures[it] = cur;
                    }
                }
            }
            it = it + 1;
        }
    }
    // 按传入顺序构造输出 Vec<F::Output>
    let mut result_vec: Vec<F::Output> = Vec::with_capacity(n);
    let mut j = 0;
    while j < n {
        match results.get(j) {
            Option::Some(x) => {
                result_vec.push(x);
            }
            Option::None => {}
        }
        j = j + 1;
    }
    result_vec
}
