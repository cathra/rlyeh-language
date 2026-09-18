// time/instant.rl：`Instant`（时刻）——2026-09-18 由 time/module.rl 拆出。
//
// 归属子模块 `time::instant`。对外 `time::Instant` 由 time/module.rl 的
// `pub import instant::Instant;` 登记重导出别名保持。
//
// 时钟 extern 声明于 core 的 externs 单元：
// - `clock()`：libc CPU 时钟（CLOCKS_PER_SEC=1e6，微秒）；
// - `__rlyeh_clock_monotonic()`：墙钟（clock_gettime CLOCK_MONOTONIC，微秒；
//   driver 注入 define，不支持平台返回 -1）。
// `now` / `elapsed` 优先墙钟（睡眠期间推进），-1 时退回 clock()。

struct Instant {
    start: i64,
}

impl Instant {
    // 当前时刻
    fn now() -> Instant {
        let t = __rlyeh_clock_monotonic();
        if t >= 0 {
            Instant { start: t }
        } else {
            Instant { start: clock() }
        }
    }
    // 距 `now` 已过时长（墙钟差，睡眠期间推进；恒 >= 0）
    fn elapsed(self) -> time::duration::Duration {
        let t = __rlyeh_clock_monotonic();
        let cur = if t >= 0 { t } else { clock() };
        time::duration::Duration { micros: cur - self.start }
    }
    // 与更早时刻的差值（X1；目标 API `duration_since(&self, earlier: Instant)`，
    // MVP 值参数简化；later - earlier，可为负——目标 API 返回 Result 规划）
    fn duration_since(self, earlier: Instant) -> time::duration::Duration {
        time::duration::Duration { micros: self.start - earlier.start }
    }
}
