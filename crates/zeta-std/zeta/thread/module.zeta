// ===== thread 模块（S 阶段，2026-08）：线程支持 =====
// S0a–S0c：`Thread::start` / `join` / `current`（std-lib.md §10）。
// 注：`spawn` 为语言保留关键字（actor 派生），线程启动方法名取 `start`。
// 实现说明：
// - 基于 driver 注入平台内建（`__zeta_thread_spawn`/`__zeta_thread_join`/
//   `__zeta_thread_self`，thread_builtin_ir，pthread 绑定），与 sendfile 相同
//   架构；WASI/Windows 下注入返回 -1 的 stub（Unsupported，禁用文档化）。
// - 线程函数须为 `fn() -> i64`（H1 函数指针值按地址整数经 extern i64 形参
//   传入，codegen 做 ptrtoint）；闭包值跨线程捕获后续支持（S3）。
// - MVP 无 TLS 需求（S0e ✅）；join 返回值经 pthread_join 返回值槽读取。
// - 默认栈大小（S0e ✅）：pthread_create attr=NULL 使用系统默认——Linux 约 8MB、
//   macOS 约 512KB（stack_size 定制规划中）；栈溢出/数据竞争属调用方责任
//   （与 C 内存模型一致）。
// - 底层 extern（__zeta_*）声明于根模块 core.zeta 的 extern 集中区。

// S0b：线程句柄（pthread_t 的 i64 视图）。
struct Thread {
    tid: i64,
}

impl Thread {
    // S0b：派生新线程运行 f（零参数、返回 i64），立即返回线程句柄。
    // 启动失败（pthread_create 非零）映射 IoError（M1b）。
    // 注意：f 须为顶层/模块级函数（函数指针）；闭包值跨线程捕获后续支持。
    fn start(f: fn() -> i64) -> Result<thread::Thread, io::error::IoError> {
        let r = __zeta_thread_spawn(f, 0);
        if r < 0 {
            Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("thread spawn failed"),
            ))
        } else {
            Result::Ok(thread::Thread { tid: r })
        }
    }

    // S0c：阻塞等待本线程结束，返回其返回值槽内容。
    // join 失败（pthread_join 非零）返回 -1（MVP 无错误通道）。
    fn join(&self) -> i64 {
        __zeta_thread_join(self.tid)
    }

    // S0e：当前线程的 pthread id（正整数；MVP 无 TLS 需求，仅作标识）。
    fn current() -> i64 {
        __zeta_thread_self()
    }
}

// S2a：阻塞当前线程指定时长（`__zeta_thread_sleep` usleep 绑定；
// micros 截断 u32，上限约 71 分钟）。返回 0 成功 / -1 失败
// （WASI/Windows 下 stub 恒 -1，禁用文档化）。
fn sleep(duration: time::Duration) -> i64 {
    __zeta_thread_sleep(duration.micros())
}

// S2b：并发等待一组线程全部完成，按传入顺序返回各线程返回值。
// 线程已由 `Thread::start` 并行派生；join_all 阻塞至全部完成
// （顺序 join 收尾——join 为阻塞原语，"全部完成才返回"语义与
// 并发收尾等价）。join 失败（pthread_join 非零）该位置返回 -1
// （MVP 无错误通道）。
fn join_all(threads: Vec<thread::Thread>) -> Vec<i64> {
    let mut results = Vec::with_capacity(threads.len());
    for t in threads {
        results.push(t.join());
    }
    results
}
