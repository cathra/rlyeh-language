// thread/builder.rl：`Builder`（线程栈大小定制）——2026-09-18 由 thread/module.rl 拆出。
//
// 归属子模块 `thread::builder`。Y8（std-lib.md §10.2）：
// - `Builder::new()` 创建默认构建器（stack_size = 0 → 系统默认栈）；
// - `stack_size(&mut self, n)` 设置线程栈字节数（须 >= PTHREAD_STACK_MIN，通常
//   16KB；超小值由 pthread_attr_setstacksize 报错，spawn 映射 IoError）；
// - `start(&self, f)` 按当前配置派生线程：走 `__rlyeh_thread_spawn_stack`
//   （pthread_attr_setstacksize 定制），stack_size <= 0 → null attr，与
//   `Thread::start` 等价。
//
// MVP 约束：`Builder` 为可变对象风格（`&mut self`），非 Rust 链式 Builder
// （`Builder::new().stack_size(n).start(f)` 链式规划中）；线程函数仍须为
// `fn() -> i64`。

struct Builder {
    stack_size: i64,
}

impl Builder {
    // Y8：创建默认线程构建器（stack_size = 0 → 系统默认栈）。
    fn new() -> Builder {
        Builder { stack_size: 0 }
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
    fn start(&self, f: fn() -> i64) -> Result<thread::handle::Thread, io::error::IoError> {
        let r = __rlyeh_thread_spawn_stack(f, 0, self.stack_size);
        if r < 0 {
            Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("thread spawn failed"),
            ))
        } else {
            Result::Ok(thread::handle::Thread { tid: r })
        }
    }
}
