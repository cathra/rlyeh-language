// thread/handle.rl：`Thread`（线程句柄）——2026-09-18 由 thread/module.rl 拆出。
//
// 归属子模块 `thread::handle`。对外 `thread::Thread` 由 thread/module.rl 的
// `pub import handle::Thread;` 保持（typecheck `check_expr/method.rs` 的线程
// 特判按短名等价匹配 `Thread`）。
//
// S0b–S0e（2026-08，std-lib.md §10）：基于 driver 注入平台内建
// （`__rlyeh_thread_spawn` / `__rlyeh_thread_join` / `__rlyeh_thread_self`，
// pthread 绑定）；WASI/Windows 注入返回 -1 的 stub（Unsupported，禁用文档化）。
// 底层 extern（`__rlyeh_*`）声明于 core 的 externs 单元。
//
// 注：`spawn` 为语言保留关键字（actor 派生），线程启动方法名取 `start`。

// S0b：线程句柄（pthread_t 的 i64 视图）。
struct Thread {
    tid: i64,
}

impl Thread {
    // S0b：派生新线程运行 f（零参数、返回 i64），立即返回线程句柄。
    // 启动失败（pthread_create 非零）映射 IoError（M1b）。
    // f 为顶层/模块级函数（函数指针）；闭包值跨线程捕获见
    // `thread::entry::__start_with_input` 与 typecheck 特判（`Thread::start(f, arg)`）。
    fn start(f: fn() -> i64) -> Result<Thread, io::error::IoError> {
        let r = __rlyeh_thread_spawn(f, 0);
        if r < 0 {
            Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("thread spawn failed"),
            ))
        } else {
            Result::Ok(Thread { tid: r })
        }
    }

    // S0c：阻塞等待本线程结束，返回其返回值槽内容。
    // join 失败（pthread_join 非零）返回 -1（MVP 无错误通道）。
    fn join(&self) -> i64 {
        __rlyeh_thread_join(self.tid)
    }

    // S0e：当前线程的 pthread id（正整数；MVP 无 TLS 需求，仅作标识）。
    fn current() -> i64 {
        __rlyeh_thread_self()
    }
}
