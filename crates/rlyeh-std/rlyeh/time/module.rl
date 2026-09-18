// time/module.rl：时间模块——只做「子模块声明 + 暴露内容导出」（2026-09-18 重整）。
//
// 组成：
//   time/duration.rl   struct Duration    + impl  →  子模块 time::duration
//   time/instant.rl    struct Instant     + impl  →  子模块 time::instant
//   time/system.rl     struct SystemTime  + impl  →  子模块 time::system
//
// `pub import` 登记 `time::Xxx → time::<mod>::Xxx` 重导出别名（typecheck
// `register_use` 的 `is_pub` 分支），使既有引用（用户代码 `time::Duration`、std
// 内部模块、标准库根 module.rl）无需改动。裸名导出见标准库根 module.rl。
//
// 底层时钟 extern 声明于 core 的 externs 单元（`clock` /
// `__rlyeh_clock_monotonic` / `__rlyeh_clock_realtime`）。
//
// module 声明顺序 = 收集期注册顺序：duration 须先于 instant（Instant::elapsed
// 返回 time::Duration）。

module duration;
pub import duration::Duration;
module instant;
pub import instant::Instant;
module system;
pub import system::SystemTime;

