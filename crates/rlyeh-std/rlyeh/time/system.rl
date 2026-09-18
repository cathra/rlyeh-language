// time/system.rl：`SystemTime`（绝对时刻）—— 归属子模块 `time::system`（2026-09-18 起
// 与 duration.rl / instant.rl 并列，由 time/module.rl 的 `pub import` 重导出）。
//
// 底层时钟 extern 声明于 core 的 externs 单元（externs/module.rl）：
// - `__rlyeh_clock_realtime()`：墙钟（clock_gettime CLOCK_REALTIME，微秒，
//   X1；driver 注入 define，不支持平台返回 -1）。
// `SystemTime` 以「距 UNIX_EPOCH 微秒」存储（1970-01-01T00:00:00Z 起算）。
// 绝对时刻跨进程可比（realtime 单调前提，NTP 调整除外）；支持平台返回 -1
// 时退回 UNIX 纪元（相对差恒为 0，见 `now`）。
// 目标 API 的 `UNIX_EPOCH` 常量依赖 const struct 字面量求值，规划中——
// MVP 以 `SystemTime::unix_epoch()` 方法提供（std-lib.md §7 注记）。

// 系统时间（UNIX 纪元起点 1970-01-01T00:00:00Z）
struct SystemTime {
    epoch_micros: i64,
}

impl SystemTime {
    // UNIX 纪元（1970-01-01T00:00:00Z；目标 API 常量 UNIX_EPOCH 规划中）
    fn unix_epoch() -> time::system::SystemTime {
        time::system::SystemTime { epoch_micros: 0 }
    }
    // 当前系统时间（clock_gettime CLOCK_REALTIME，X1；不支持平台返回 UNIX 纪元）
    fn now() -> time::system::SystemTime {
        let t = __rlyeh_clock_realtime();
        if t >= 0 {
            time::system::SystemTime { epoch_micros: t }
        } else {
            time::system::SystemTime { epoch_micros: 0 }
        }
    }
    // 与更早时刻的差值（later - earlier，可为负——目标 API 返回
    // Result<Duration, TimeError> 规划中，MVP 直接差值）
    fn duration_since(self, earlier: time::system::SystemTime) -> time::duration::Duration {
        time::duration::Duration { micros: self.epoch_micros - earlier.epoch_micros }
    }
}
