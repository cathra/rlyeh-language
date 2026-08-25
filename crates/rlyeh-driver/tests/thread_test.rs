//! S 阶段（2026-08）线程支持集成测试（自动注入 `rlyeh-std/rlyeh/core.rl`）。
//!
//! 覆盖：
//! - `Thread::start(f: fn() -> i64)`：派生线程运行零参数函数（H1 函数指针值
//!   经 extern i64 形参按地址整数传递，codegen ptrtoint + driver 注入
//!   `__rlyeh_thread_spawn` pthread 绑定；`spawn` 为保留关键字，方法名取 start）
//! - `Thread::join()`：阻塞等待 + 返回值槽读取（pthread_join）
//! - 并发执行验证：两个线程 join 求和
//! - `Thread::current()`：当前线程 id 为正整数（pthread_self）
//! - Y8：`Builder::stack_size(&mut self, n)` + `Builder::start(&self, f)`：
//!   定制线程栈（pthread_attr_setstacksize）——64MB 栈跑 100000 层深递归
//!   （默认栈 macOS 约 512KB / Linux 约 8MB 必 SIGSEGV，可区分）；
//!   `Builder::new()` 默认（stack_size = 0 → null attr，与 start 等价）。
//!   注：`spawn` 为保留关键字（actor 派生），Builder 定制启动方法名取 `start`。
//!
//! 需要系统 clang（与 nio_test.rs / net_socket_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-thread-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.rl），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("线程模块测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// `Thread::start` 派生线程 + `join` 读取返回值。
/// worker 打印 41 后返回 42；main 在 join 前打印 1（线程调度不确定，
/// 41 与 1 顺序可变，但 join 返回值 42 必为最后一行）。
#[test]
fn spawn_join_return() {
    let out = run(
        r#"
fn worker() -> i64 {
    println(41);
    42
}
fn main() {
    match Thread::start(worker) {
        Ok(t) => {
            println(1);
            let r = t.join();
            println(r);
        },
        Err(_) => println(-1),
    }
}
"#,
    );
    assert!(out.ends_with("42\n"), "out={out:?}");
    assert!(out.contains("41\n"), "out={out:?}");
    assert!(out.contains("1\n"), "out={out:?}");
}

/// 两个线程并行运行，join 求和（并发执行验证）。
#[test]
fn spawn_two_threads_sum() {
    let out = run(
        r#"
fn w1() -> i64 { 10 }
fn w2() -> i64 { 20 }
fn main() {
    let t1 = Thread::start(w1);
    let t2 = Thread::start(w2);
    match t1 {
        Ok(a) => {
            match t2 {
                Ok(b) => {
                    let s = a.join() + b.join();
                    println(s);
                },
                Err(_) => println(-1),
            }
        },
        Err(_) => println(-1),
    }
}
"#,
    );
    assert_eq!(out, "30\n");
}

/// `Thread::current()` 返回正整数线程 id（pthread_self）。
#[test]
fn current_positive() {
    let out = run(
        r#"
fn main() {
    let id = Thread::current();
    if id > 0 {
        println(1);
    } else {
        println(-1);
    }
}
"#,
    );
    assert_eq!(out, "1\n");
}

/// `thread::sleep(Duration)` 阻塞当前线程（S2a，usleep 绑定），返回 0 成功。
/// 注：`Instant::elapsed` 基于 `clock()`（CPU 时钟），睡眠期间不推进，
/// 故仅断言返回值 0，不依赖时长推进。
#[test]
fn sleep_duration_ok() {
    let out = run(
        r#"
fn main() {
    let r = sleep(Duration::milliseconds(20));
    println(r);
}
"#,
    );
    assert_eq!(out, "0\n");
}

/// `thread::join_all`：并发等待多线程完成，按传入顺序返回各线程返回值（S2b）。
/// worker 无 sleep（纯计算），join_all 收集 [10, 20, 30] 求和 60。
#[test]
fn join_all_returns_in_order() {
    let out = run(
        r#"
fn w1() -> i64 { 10 }
fn w2() -> i64 { 20 }
fn w3() -> i64 { 30 }
fn main() {
    let t1 = Thread::start(w1);
    let t2 = Thread::start(w2);
    let t3 = Thread::start(w3);
    match t1 {
        Ok(a) => match t2 {
            Ok(b) => match t3 {
                Ok(c) => {
                    let mut ts: Vec<thread::Thread> = Vec::with_capacity(3);
                    ts.push(a);
                    ts.push(b);
                    ts.push(c);
                    let rs = join_all(ts);
                    println(rs[0] + rs[1] + rs[2]);
                },
                Err(_) => println(-1),
            },
            Err(_) => println(-2),
        },
        Err(_) => println(-3),
    }
}
"#,
    );
    assert_eq!(out, "60\n");
}

/// 墙钟（clock_gettime CLOCK_MONOTONIC，S2b 接入）：sleep 期间
/// `Instant::elapsed` 推进（CPU 时钟下睡眠不推进，此为墙钟验证）。
#[test]
fn wall_clock_elapsed_advances_during_sleep() {
    let out = run(
        r#"
fn main() {
    let t0 = Instant::now();
    sleep(Duration::milliseconds(30));
    let d = t0.elapsed();
    if d.millis() >= 30 {
        println(1);
    } else {
        println(-1);
    }
}
"#,
    );
    assert_eq!(out, "1\n");
}

/// Y8：`Builder::stack_size` 定制线程栈（pthread_attr_setstacksize）。
/// 深递归（100000 层，需求 > 默认栈：macOS 约 512KB / Linux 约 8MB）在
/// 64MB 定制栈下正常完成——若 attr 未生效（仍用默认栈）测试进程将 SIGSEGV，
/// 故本测试同时正证定制栈生效。返回 rec(100000) = 100000。
/// 注：`Builder::start` 线程函数须为 `fn() -> i64`（无参），带参 `rec(n)`
/// 经零参数包装函数 `run_rec` 适配（H1 函数指针约束，与 Thread::start 一致）。
#[test]
fn builder_stack_size_deep_recursion() {
    let out = run(
        r#"
fn rec(n: i64) -> i64 {
    if n <= 0 {
        0
    } else {
        rec(n - 1) + 1
    }
}
fn run_rec() -> i64 {
    rec(100000)
}
fn main() {
    let mut b = Builder::new();
    let r = b.stack_size(64 * 1024 * 1024);
    if r != 0 {
        println(-2);
    } else {
        match b.start(run_rec) {
            Ok(t) => println(t.join()),
            Err(_) => println(-1),
        }
    }
}
"#,
    );
    assert_eq!(out, "100000\n");
}

/// Y8：`Builder::new()` 默认构建器（stack_size = 0 → 系统默认栈，
/// 走 null attr 路径，与 `Thread::start` 等价）。
#[test]
fn builder_default_stack_spawn() {
    let out = run(
        r#"
fn w() -> i64 { 42 }
fn main() {
    // 对照：Builder 定制启动 vs Thread::start
    match Thread::start(w) {
        Ok(t) => println(t.join()),
        Err(_) => println(-1),
    }
    let mut b = Builder::new();
    let r = b.stack_size(0);
    if r != 0 {
        println(-2);
    } else {
        match b.start(w) {
            Ok(t) => println(t.join()),
            Err(_) => println(-1),
        }
    }
}
"#,
    );
    assert_eq!(out, "42\n42\n");
}
