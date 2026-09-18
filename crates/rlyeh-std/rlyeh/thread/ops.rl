// thread/ops.rl：线程相关自由函数（`sleep` / `join_all`）——2026-09-18 由 thread/module.rl 拆出。
//
// 归属子模块 `thread::ops`；对外函数名由 thread/module.rl 的 `pub import` 保持。

// S2a：阻塞当前线程指定时长（`__rlyeh_thread_sleep` usleep 绑定；
// micros 截断 u32，上限约 71 分钟）。返回 0 成功 / -1 失败
// （WASI/Windows 下 stub 恒 -1，禁用文档化）。
fn sleep(duration: time::duration::Duration) -> i64 {
    __rlyeh_thread_sleep(duration.micros())
}

// S2b：并发等待一组线程全部完成，按传入顺序返回各线程返回值。
// 线程已由 `Thread::start` 并行派生；join_all 阻塞至全部完成
// （顺序 join 收尾——join 为阻塞原语，"全部完成才返回"语义与并发收尾等价）。
// join 失败（pthread_join 非零）该位置返回 -1（MVP 无错误通道）。
fn join_all(threads: Vec<thread::handle::Thread>) -> Vec<i64> {
    let mut results = Vec::with_capacity(threads.len());
    for t in threads {
        results.push(t.join());
    }
    results
}
