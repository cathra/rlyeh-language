//! `Mutex` / `RwLock` 同步模块集成测试（文件入口 API，自动注入
//! `rlyeh-std/rlyeh/core.rl`；基于 extern FFI 绑定 pthread，原语对象承载于
//! malloc 缓冲）。
//!
//! 单线程下利用 `trylock` 的 EBUSY 语义真实验证互斥性：
//! - Mutex：未锁 try_lock 成功、已锁 try_lock 失败（EBUSY）、unlock 后恢复
//! - RwLock：读-读共享、读-写/写-写互斥（try 系列返回 EBUSY）
//! - lock/unlock 临界区保护与循环稳定性
//!
//! 需要系统 clang（与 std_test.rs / agg_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-sync-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.rl），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("sync 模块测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// Mutex try_lock 的 EBUSY 语义（真实互斥性验证）。
#[test]
fn mutex_trylock_ebusy_semantics() {
    let out = run(
        r#"
fn main() {
    let m = Mutex::new(0);
    // 未加锁：try_lock 成功
    if m.try_lock() { println(1); } else { println(0); }   // 1
    // 已加锁：try_lock 失败（EBUSY）
    if m.try_lock() { println(1); } else { println(0); }   // 0
    // 解锁后恢复
    m.unlock();
    if m.try_lock() { println(1); } else { println(0); }   // 1
    m.unlock();
}
"#,
    );
    assert_eq!(out, "1\n0\n1\n");
}

/// Mutex lock/unlock 保护临界区：计数在锁内自增，解锁后继续。
#[test]
fn mutex_lock_unlock_critical_section() {
    let out = run(
        r#"
fn main() {
    let m = Mutex::new(0);
    let mut count = 0;
    let mut i = 0;
    while i < 10 {
        m.lock();               // 进入临界区
        count = count + 1;      // 临界区操作
        m.unlock();             // 离开临界区
        i = i + 1;
    }
    println(count);             // 10
}
"#,
    );
    assert_eq!(out, "10\n");
}

/// Mutex 大量 lock/unlock 循环稳定性。
#[test]
fn mutex_loop_stability() {
    let out = run(
        r#"
fn main() {
    let m = Mutex::new(0);
    let mut i = 0;
    while i < 100 {
        m.lock();
        m.unlock();
        i = i + 1;
    }
    println(100);
}
"#,
    );
    assert_eq!(out, "100\n");
}

/// P5（2026-08-28）：带值锁 + lock_guard 访问——`Mutex::new(v)` 带值，守卫
/// `get`/`get_mut` 经引用读写被锁值，函数尾自动解锁。
#[test]
fn mutex_value_lock_guard() {
    let out = run(
        r#"
fn main() {
    let mut m = Mutex::new(42);
    let g = m.lock_guard();
    println(*g.get());      // 42（带值锁读）
    let mut m2 = Mutex::new(10);
    let mut g2 = m2.lock_guard();
    *g2.get_mut() = 77;     // 写回
    println(*g2.get());     // 77
    println(m2.value);      // 77（写回生效）
}
"#,
    );
    assert_eq!(out, "42\n77\n77\n");
}

/// RwLock 读-读共享、读锁下写锁互斥（EBUSY）。
#[test]
fn rwlock_read_read_not_exclusive() {
    let out = run(
        r#"
fn main() {
    let r = RwLock::new();
    // 第一个读锁：成功
    if r.try_read_lock() { println(1); } else { println(0); }   // 1
    // 读锁下再取读锁：读读共享，仍成功
    if r.try_read_lock() { println(1); } else { println(0); }   // 1
    // 读锁下取写锁：互斥，失败（EBUSY）
    if r.try_write_lock() { println(1); } else { println(0); }  // 0
    // 释放两个读锁后，写锁可获取
    r.unlock();
    r.unlock();
    if r.try_write_lock() { println(1); } else { println(0); }  // 1
    r.unlock();
}
"#,
    );
    assert_eq!(out, "1\n1\n0\n1\n");
}

/// RwLock 写锁排他（写-写互斥、写锁下读锁互斥）。
#[test]
fn rwlock_write_exclusive() {
    let out = run(
        r#"
fn main() {
    let r = RwLock::new();
    // 第一个写锁：成功
    if r.try_write_lock() { println(1); } else { println(0); }  // 1
    // 写锁下再取写锁：互斥，失败
    if r.try_write_lock() { println(1); } else { println(0); }  // 0
    // 写锁下取读锁：互斥，失败
    if r.try_read_lock() { println(1); } else { println(0); }   // 0
    // 释放写锁后，读锁可获取
    r.unlock();
    if r.try_read_lock() { println(1); } else { println(0); }   // 1
    r.unlock();
}
"#,
    );
    assert_eq!(out, "1\n0\n0\n1\n");
}

/// RwLock read_lock/write_lock 交替循环（阻塞版 API 稳定性）。
#[test]
fn rwlock_read_write_loop() {
    let out = run(
        r#"
fn main() {
    let r = RwLock::new();
    let mut i = 0;
    while i < 5 {
        r.read_lock();
        r.unlock();
        r.write_lock();
        r.unlock();
        i = i + 1;
    }
    println(5);
}
"#,
    );
    assert_eq!(out, "5\n");
}
