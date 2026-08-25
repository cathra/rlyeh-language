// ===== 时间模块（MVP）：Duration / Instant =====
// 底层时钟 extern 声明于根模块 core.rl 的 extern 集中区：
// - `clock()`：libc CPU 时钟（POSIX CLOCKS_PER_SEC=1e6，返回微秒）。
// - `__rlyeh_clock_monotonic()`：墙钟（clock_gettime CLOCK_MONOTONIC，微秒，
//   S2b；driver 注入 define，不支持平台返回 -1）。
// `Instant::now/elapsed` 优先墙钟（睡眠期间推进），-1 时退回 clock()。
// 语言层无常量定义，1e6 以字面量 1000000 使用。
// 目录化（2026-08）：time/module.rl = 原 time.rl（模块规模小，保持单文件）。

// 时长：以微秒为唯一存储单位，各单位视图由换算方法提供。
struct Duration {
    micros: i64,
}

impl Duration {
    // 构造器：秒（std-lib.md §8 规划 API；u64 → i64 实现，溢出未检查）
    fn seconds(n: i64) -> time::Duration {
        time::Duration { micros: n * 1000000 }
    }
    // 构造器：毫秒
    fn milliseconds(n: i64) -> time::Duration {
        time::Duration { micros: n * 1000 }
    }
    // 秒（向下取整）
    fn secs(self) -> i64 {
        self.micros / 1000000
    }
    // 毫秒
    fn millis(self) -> i64 {
        self.micros / 1000
    }
    // 微秒
    fn micros(self) -> i64 {
        self.micros
    }
    // 纳秒
    fn nanos(self) -> i64 {
        self.micros * 1000
    }
}

// 时刻：记录起始微秒，`elapsed` 计算与当前时刻的差值。
struct Instant {
    start: i64,
}

impl Instant {
    // 当前时刻（墙钟：clock_gettime CLOCK_MONOTONIC，S2b；不支持平台退回 CPU 时钟）
    fn now() -> time::Instant {
        let t = __rlyeh_clock_monotonic();
        if t >= 0 {
            time::Instant { start: t }
        } else {
            time::Instant { start: clock() }
        }
    }
    // 距 `now` 已过时长（墙钟差，睡眠期间推进；恒 >= 0）
    fn elapsed(self) -> time::Duration {
        let t = __rlyeh_clock_monotonic();
        let cur = if t >= 0 { t } else { clock() };
        time::Duration { micros: cur - self.start }
    }
}
