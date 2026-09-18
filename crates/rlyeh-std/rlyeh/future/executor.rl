// future/executor.rl：执行器（`block_on` / `timeout` / `join_all`）——2026-09-18 由 future/module.rl 拆出。
//
// 归属子模块 `future::executor`；对外函数名由 future/module.rl 的 `pub import` 保持。
//
// - `block_on`（S1b/W3）：阻塞轮询直到 `Ready`，返回 `F::Output`（W4 关联类型投影）；
//   事件驱动：`Pending` 且 future 经 `cx.deadline` 请求唤醒时刻时休眠到该时刻（不忙等），
//   `cx.fd` 请求 fd 事件时经 `Poller` 等待（W3 第二步）；
// - `timeout`（S2c/W4）：`duration` 内未 `Ready` 返回 `Err(TimeoutError)`；
// - `join_all`（W4）：并发轮询多个 Future 直至全部 `Ready`，按传入顺序收集输出。
//
// 注：泛型约束 `F: Future` 的 bound 解析宽松（实例化时按具体类型解析 `poll`）；
// MVP 静态方法调用不支持模块路径前缀（如 `time::instant::Instant::now` 不可用），
// 故时钟直接复用 `__rlyeh_clock_monotonic` / `clock()` extern，定时休眠经 `thread::sleep`。

// S1b/W3：阻塞轮询 `f` 直到 `poll` 返回 `Ready`，返回其携带值（`F::Output`）。
// 调用方把状态机实现为 `Future` 类型后 `block_on(&mut fut)`；泛型实参由实参类型推断
// （MVP 实例化时重查 body，`f.poll()` 解析到具体 impl）。
fn block_on<F: Future>(f: &mut F) -> F::Output {
    let mut cx = future::poll::Context { _unit: 0, deadline: 0, fd: 0, interest: 0 };
    loop {
        match f.poll(&mut cx) {
            future::poll::Poll::Ready(v) => return v,
            future::poll::Poll::Pending => {
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

// S2c/W4：带超时阻塞轮询——`duration` 内未 `Ready` 返回 `Err(TimeoutError)`（超时）。
// 对齐 std-lib §10 规划 API 参数顺序（duration 在前）与输出
// `Result<F::Output, TimeoutError>`——`F::Output` 关联类型投影落地，输出不再限 `i64`。
fn timeout<F: Future>(duration: time::duration::Duration, f: &mut F) -> Result<F::Output, future::error::TimeoutError> {
    let limit = duration.micros();
    let t0 = __rlyeh_clock_monotonic();
    let start = if t0 >= 0 { t0 } else { clock() };
    let mut cx = future::poll::Context { _unit: 0, deadline: 0, fd: 0, interest: 0 };
    loop {
        match f.poll(&mut cx) {
            future::poll::Poll::Ready(v) => return Result::Ok(v),
            future::poll::Poll::Pending => {
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
// 对齐 std-lib.md §10 规划 `join_all<F: Future>(Vec<F>) -> Vec<F::Output>`。
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
        let mut cx = future::poll::Context { _unit: 0, deadline: 0, fd: 0, interest: 0 };
        let mut it = 0;
        while it < n {
            if done[it] == 0 {
                let mut cur = futures[it];
                match cur.poll(&mut cx) {
                    future::poll::Poll::Ready(v) => {
                        results.insert(it, v);
                        finished = finished + 1;
                        futures[it] = cur;
                        done[it] = 1;
                    }
                    future::poll::Poll::Pending => {
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
