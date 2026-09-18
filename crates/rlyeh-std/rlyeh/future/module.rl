// future/module.rl：异步运行时基础——只做「子模块声明 + 暴露内容导出」（2026-09-18 重整）。
//
// 组成（S1/S2 + W1/W3/W4）：
//   future/poll.rl       enum Poll + struct Context          → 子模块 future::poll
//   future/interface.rl   protocol Future（关联类型 Output）  → 子模块 future::protocol
//   future/error.rl      struct TimeoutError + impl          → 子模块 future::error
//   future/sleep.rl      struct Sleep + fn sleep             → 子模块 future::sleep
//   future/wait_fd.rl    struct WaitFd + fn wait_fd          → 子模块 future::wait_fd
//   future/executor.rl   block_on / timeout / join_all       → 子模块 future::executor
//
// `pub import` 登记 `future::Xxx → future::<mod>::Xxx` 重导出别名，使既有引用无需改动：
// 用户代码 `future::Sleep` / `future::sleep` / `future::join_all`、std 内 net/http 与
// sync 的裸名 `Future`/`Poll`/`Context`（经标准库根导出）、async desugar。
//
// module 声明顺序 = 收集期注册顺序：poll / protocol / error 先于 sleep / wait_fd /
// executor（后者签名引用 `future::poll::*`、`future::interface::Future`、
// `future::error::TimeoutError`）。

module poll;
pub import poll::Poll;
pub import poll::Context;
module interface;
pub import interface::Future;
module error;
pub import error::TimeoutError;
module sleep;
pub import sleep::Sleep;
pub import sleep::sleep;
module wait_fd;
pub import wait_fd::WaitFd;
pub import wait_fd::wait_fd;
module executor;
pub import executor::block_on;
pub import executor::timeout;
pub import executor::join_all;
