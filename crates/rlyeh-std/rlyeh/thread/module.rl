// ===== thread 模块（S 阶段，2026-08）：线程支持 =====
// S0a–S0c：`Thread::start` / `join` / `current`（std-lib.md §10）。
// 注：`spawn` 为语言保留关键字（actor 派生），线程启动方法名取 `start`。
// 实现说明：
// - 基于 driver 注入平台内建（`__rlyeh_thread_spawn`/`__rlyeh_thread_join`/
//   `__rlyeh_thread_self`，thread_builtin_ir，pthread 绑定），与 sendfile 相同
//   架构；WASI/Windows 下注入返回 -1 的 stub（Unsupported，禁用文档化）。
// - 线程函数须为 `fn() -> i64`（H1 函数指针值按地址整数经 extern i64 形参
//   传入，codegen 做 ptrtoint）；闭包值跨线程捕获后续支持（S3）。
// - MVP 无 TLS 需求（S0e ✅）；join 返回值经 pthread_join 返回值槽读取。
// - 默认栈大小（S0e ✅）：pthread_create attr=NULL 使用系统默认——Linux 约 8MB、
//   macOS 约 512KB（stack_size 定制规划中）；栈溢出/数据竞争属调用方责任
//   （与 C 内存模型一致）。
// - 底层 extern（__rlyeh_*）声明于根模块 core.rl 的 extern 集中区。

// S0b：线程句柄（pthread_t 的 i64 视图）。
struct Thread {
    tid: i64,
}

impl Thread {
    // S0b：派生新线程运行 f（零参数、返回 i64），立即返回线程句柄。
    // 启动失败（pthread_create 非零）映射 IoError（M1b）。
    // 注意：f 须为顶层/模块级函数（函数指针）；闭包值跨线程捕获后续支持。
    fn start(f: fn() -> i64) -> Result<thread::Thread, io::error::IoError> {
        let r = __rlyeh_thread_spawn(f, 0);
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
        __rlyeh_thread_join(self.tid)
    }

    // S0e：当前线程的 pthread id（正整数；MVP 无 TLS 需求，仅作标识）。
    fn current() -> i64 {
        __rlyeh_thread_self()
    }
}

// Y8：`Builder`——线程栈大小定制（std-lib.md §10.2）。
// - `Builder::new()` 创建默认构建器（stack_size = 0 → 系统默认栈）；
// - `stack_size(&mut self, n)` 设置线程栈字节数（须 >= PTHREAD_STACK_MIN，
//   通常 16KB；超小值由 pthread_attr_setstacksize 报错，spawn 映射 IoError）；
// - `spawn(&self, f)` 按当前配置派生线程：走 `__rlyeh_thread_spawn_stack`
//   （pthread_attr_setstacksize 定制），stack_size <= 0 → null attr，
//   与 `Thread::start` 等价。
// MVP 约束：`Builder` 为可变对象风格（`&mut self`），非 Rust 链式 Builder
//   （`Builder::new().stack_size(n).spawn(f)` 链式规划中）；线程函数仍须为
//   `fn() -> i64`（同 Thread::start，闭包值跨线程捕获后续支持）。
struct Builder {
    stack_size: i64,
}

impl Builder {
    // Y8：创建默认线程构建器（stack_size = 0 → 系统默认栈）。
    fn new() -> thread::Builder {
        thread::Builder { stack_size: 0 }
    }

    // Y8：设置线程栈字节数（<=0 表示使用系统默认栈）。
    fn stack_size(&mut self, n: i64) -> i64 {
        self.stack_size = n;
        0
    }

    // Y8：按当前配置派生线程运行 f（零参数、返回 i64），立即返回线程句柄。
    // 启动失败（pthread_create / attr_setstacksize 非零）映射 IoError（M1b）。
    // 注：`spawn` 为保留关键字（actor 派生），定制启动方法名取 `start`
    //（与 Thread::start 命名一致）。
    fn start(&self, f: fn() -> i64) -> Result<thread::Thread, io::error::IoError> {
        let r = __rlyeh_thread_spawn_stack(f, 0, self.stack_size);
        if r < 0 {
            Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("thread spawn failed"),
            ))
        } else {
            Result::Ok(thread::Thread { tid: r })
        }
    }
}

// S2a：阻塞当前线程指定时长（`__rlyeh_thread_sleep` usleep 绑定；
// micros 截断 u32，上限约 71 分钟）。返回 0 成功 / -1 失败
// （WASI/Windows 下 stub 恒 -1，禁用文档化）。
fn sleep(duration: time::Duration) -> i64 {
    __rlyeh_thread_sleep(duration.micros())
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
