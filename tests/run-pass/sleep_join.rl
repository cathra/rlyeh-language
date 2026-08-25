// S2a（2026-08）：`thread::sleep(Duration)` 阻塞当前线程
// （`__rlyeh_thread_sleep` usleep 绑定；micros 截断 u32，上限约 71 分钟）。
// 注：`Instant::now/elapsed` 基于 `clock()`（CPU 时钟），睡眠期间不推进，
// 故断言 sleep 返回值 0 + 线程存活，不依赖时长推进（S2b 起可换墙钟）。
fn main() -> i64 {
    let r = sleep(Duration::milliseconds(20));
    if r == 0 && Thread::current() > 0 {
        println("sleep-ok");
    } else {
        println("sleep-fail");
    }
    0
}
