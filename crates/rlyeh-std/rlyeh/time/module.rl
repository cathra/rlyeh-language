// ===== 时间模块（MVP + X1）：Duration / Instant / SystemTime =====
// 底层时钟 extern 声明于根模块 core.rl 的 extern 集中区：
// - `clock()`：libc CPU 时钟（POSIX CLOCKS_PER_SEC=1e6，返回微秒）。
// - `__rlyeh_clock_monotonic()`：墙钟（clock_gettime CLOCK_MONOTONIC，微秒，
//   S2b；driver 注入 define，不支持平台返回 -1）。
// - `__rlyeh_clock_realtime()`：系统时间（clock_gettime CLOCK_REALTIME，微秒，
//   X1；driver 注入 define，不支持平台返回 -1）。
// `Instant::now/elapsed` 优先墙钟（睡眠期间推进），-1 时退回 clock()。
// 语言层无常量定义，1e6 以字面量 1000000 使用。
// 目录化（2026-08）：time/module.rl = 原 time.rl；SystemTime 见 time/system.rl。
// X1（2026-08-25）：补目标 API 构造器 microseconds/nanoseconds 与读方法
// as_secs/as_millis/as_nanos（u64/u128 → i64 实现，溢出未检查）+ Instant::duration_since。
// U6（2026-08-25）：`as` 转换 IR 落地，`Duration::from_secs_f64` 解锁
// （`(secs * 1e6) as i64`，fptosi 向零截断）。
// module 声明（system）置于文件末尾——收集期注册顺序为 module.rl 顶层 item
// （Duration/Instant）在前、time/system.rl（SystemTime）在后（io 惯例）。

// 时长：以微秒为唯一存储单位，各单位视图由换算方法提供。
struct Duration {
    micros: i64,
}

impl Duration {
    // 构造器：秒（std-lib.md §7 目标 API；u64 → i64 实现，溢出未检查）
    fn seconds(n: i64) -> time::Duration {
        time::Duration { micros: n * 1000000 }
    }
    // 构造器：毫秒
    fn milliseconds(n: i64) -> time::Duration {
        time::Duration { micros: n * 1000 }
    }
    // 构造器：微秒（X1；u64 → i64 实现，溢出未检查）
    fn microseconds(n: i64) -> time::Duration {
        time::Duration { micros: n }
    }
    // 构造器：纳秒（X1；以微秒存储，向下取整——与 Rust 精确纳秒存储不同，
    // 见 std-lib.md §7 注记；u64 → i64 实现）
    fn nanoseconds(n: i64) -> time::Duration {
        time::Duration { micros: n / 1000 }
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
    // 秒（X1；目标 API `as_secs`，u64 → i64 实现；`secs` 保留兼容别名）
    fn as_secs(self) -> i64 {
        self.micros / 1000000
    }
    // 毫秒（X1；目标 API `as_millis`，u64 → i64 实现；`millis` 保留兼容别名）
    fn as_millis(self) -> i64 {
        self.micros / 1000
    }
    // 纳秒（X1；目标 API `as_nanos`，u128 → i64 实现，超 2^63 纳秒溢出未检查；
    // `nanos` 保留兼容别名）
    fn as_nanos(self) -> i64 {
        self.micros * 1000
    }
    // 秒（浮点，U6 Cast IR 解锁：`secs * 1e6 as i64`，fptosi 向零截断）
    fn from_secs_f64(secs: f64) -> time::Duration {
        time::Duration { micros: (secs * 1000000.0) as i64 }
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
    // 与更早时刻的差值（X1；目标 API `duration_since(&self, earlier: Instant)`，
    // MVP 值参数简化；later - earlier，可为负——目标 API 返回 Result 规划）
    fn duration_since(self, earlier: time::Instant) -> time::Duration {
        time::Duration { micros: self.start - earlier.start }
    }
}

// module 声明顺序即收集期注册顺序（io 惯例）：module.rl 顶层 item 在前，
// 子模块在后——SystemTime 引用父模块 time::Duration 须在其注册之后。
module system;
