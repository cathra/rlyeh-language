// future/sleep.rl：定时器 future（`Sleep` / `sleep`）——2026-09-18 由 future/module.rl 拆出。
//
// 归属子模块 `future::sleep`。对外 `future::Sleep` / `future::sleep` 由
// future/module.rl 的 `pub import` 保持（用户代码 `let s: future::Sleep =
// future::sleep(d)` 经该别名链解析）。

// W3（2026-08-25）：定时器 future——在 `duration` 内 `Pending`（向 `cx.deadline`
// 请求唤醒时刻），到时 `Ready(0)`。供事件驱动 executor 休眠而非忙等；经 `&mut *cx`
// 透传写 deadline（与子 future 共享同一 Context 实例，外层/executor 可读）。
struct Sleep {
    target: i64,
}

fn sleep(duration: time::duration::Duration) -> future::sleep::Sleep {
    let d = duration.micros();
    let t0 = __rlyeh_clock_monotonic();
    let start = if t0 >= 0 { t0 } else { clock() };
    Sleep { target: start + d }
}

impl Sleep: Future {
    type Output = i64;
    fn poll(&mut self, cx: &mut future::poll::Context) -> future::poll::Poll<Self::Output> {
        let t0 = __rlyeh_clock_monotonic();
        let now = if t0 >= 0 { t0 } else { clock() };
        if now >= self.target {
            future::poll::Poll::Ready(0)
        } else {
            cx.deadline = self.target;
            future::poll::Poll::Pending
        }
    }
}
