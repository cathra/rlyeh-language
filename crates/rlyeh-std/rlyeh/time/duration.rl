// time/duration.rl：`Duration`（时长，微秒存储）——2026-09-18 由 time/module.rl 拆出。
//
// 归属子模块 `time::duration`。对外的 `time::Duration` 全名由 time/module.rl 的
// `pub import duration::Duration;` 登记重导出别名保持（typecheck `register_use`），
// 故既有引用（用户代码 `time::Duration`、std 内部模块）无需改动。
//
// 存储：以微秒为唯一单位，各单位视图由换算方法提供；u64/u128 目标 API 以 i64
// 实现（溢出未检查）。

struct Duration {
    micros: i64,
}

impl Duration {
    // 构造器：秒
    fn seconds(n: i64) -> Duration {
        Duration { micros: n * 1000000 }
    }
    // 构造器：毫秒
    fn milliseconds(n: i64) -> Duration {
        Duration { micros: n * 1000 }
    }
    // 构造器：微秒（X1）
    fn microseconds(n: i64) -> Duration {
        Duration { micros: n }
    }
    // 构造器：纳秒（X1；以微秒存储向下取整，与 Rust 精确纳秒存储不同）
    fn nanoseconds(n: i64) -> Duration {
        Duration { micros: n / 1000 }
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
    // 秒（X1；目标 API `as_secs`，`secs` 保留兼容别名）
    fn as_secs(self) -> i64 {
        self.micros / 1000000
    }
    // 毫秒（X1；目标 API `as_millis`，`millis` 保留兼容别名）
    fn as_millis(self) -> i64 {
        self.micros / 1000
    }
    // 纳秒（X1；目标 API `as_nanos`，超 2^63 纳秒溢出未检查；`nanos` 保留兼容别名）
    fn as_nanos(self) -> i64 {
        self.micros * 1000
    }
    // 秒（浮点，U6 Cast IR 解锁：fptosi 向零截断）
    fn from_secs_f64(secs: f64) -> Duration {
        Duration { micros: (secs * 1000000.0) as i64 }
    }
}
