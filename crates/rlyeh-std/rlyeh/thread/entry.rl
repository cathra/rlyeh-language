// thread/entry.rl：闭包跨线程捕获的底层辅助 —— 2026-09-18 由 thread/module.rl 拆出。
//
// 归属子模块 `thread::entry`。W6（2026-08-26）：`Thread::start(f, arg)`（f 为带参
// 闭包值对象）由 typecheck 特判展开——生成线程入口 thunk + 线程输入聚合对象
// （闭包捕获槽值 + arg），再调用本函数（Result 构造复用 std 语言层）。
//
// F-M4：`Thread::start(move || ..)`（零参 move 闭包）捕获环境按 `'static` 约束
// 校验后堆分配（捕获借用引用 &T 会被拒绝）。
//
// 对外的 `thread::__start_with_input` 全名由 thread/module.rl 的 `pub import`
// 保持；编译器经 `resolve_full_name` 定位（check_expr/method/thread.rs）。

fn __start_with_input(entry_fn: fn(i64) -> i64, input: i64) -> Result<thread::handle::Thread, io::error::IoError> {
    let r = __rlyeh_thread_spawn(entry_fn, input);
    if r < 0 {
        Result::Err(IoError::new(
            io::error::IoErrorKind::Other,
            String::from("thread spawn failed"),
        ))
    } else {
        Result::Ok(thread::handle::Thread { tid: r })
    }
}
