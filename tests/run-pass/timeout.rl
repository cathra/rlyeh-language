// timeout.rl：S2c 带超时轮询（Future 超时包装）
// - 成功路径：MyFut 三轮 Pending 后 Ready(3)，100ms 时限内完成 → Ok(3)
// - 超时路径：NeverFut 恒 Pending，50ms 时限到期 → Err(TimeoutError)
// 超时判定经墙钟（S2b ✅ clock_gettime MONOTONIC）。W4：Err 携带 TimeoutError。
struct MyFut {
    state: i64,
}

impl Future for MyFut {
    type Output = i64;
    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output> {
        self.state += 1;
        if self.state >= 3 {
            Poll::Ready(self.state)
        } else {
            Poll::Pending
        }
    }
}

struct NeverFut {
    dummy: i64,
}

impl Future for NeverFut {
    type Output = i64;
    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output> {
        Poll::Pending
    }
}

fn main() {
    // 成功路径：三轮后 Ready(3)，timeout 返回 Ok(3)
    let mut f1 = MyFut { state: 0 };
    match timeout(Duration::milliseconds(100), &mut f1) {
        Result::Ok(v) => println(v),
        Result::Err(_) => println(-1),
    }
    // 超时路径：恒 Pending，50ms 到期返回 Err(TimeoutError)
    let mut f2 = NeverFut { dummy: 0 };
    match timeout(Duration::milliseconds(50), &mut f2) {
        Result::Ok(v) => println(v),
        Result::Err(e) => println(e.message()),
    }
}
