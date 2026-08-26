//! `Duration` / `Instant` 时间模块集成测试（文件入口 API，自动注入
//! `rlyeh-std/rlyeh/core.rl`；底层时钟为 libc `clock()`，POSIX CLOCKS_PER_SEC=1e6）。
//!
//! 覆盖：Duration 各单位换算（确定性）、Instant::now/elapsed（CPU 时钟差
//! 恒 >= 0）、字段构造与字段访问、参与算术。
//!
//! 需要系统 clang（与 std_test.rs / agg_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-time-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.rl），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("time 模块测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// Duration 单位换算（确定性）。
#[test]
fn duration_unit_conversion() {
    let out = run(
        r#"
fn main() {
    let d = Duration { micros: 5000000 };
    println(d.secs());      // 5
    println(d.millis());    // 5000
    println(d.micros());    // 5000000
    println(d.nanos());     // 5000000000
    // 非整秒
    let d2 = Duration { micros: 1500000 };
    println(d2.secs());     // 1（向下取整）
    println(d2.millis());   // 1500
    println(d2.nanos());    // 1500000000
    // 零值
    let z = Duration { micros: 0 };
    println(z.secs());      // 0
    println(z.micros());    // 0
}
"#,
    );
    assert_eq!(out, "5\n5000\n5000000\n5000000000\n1\n1500\n1500000000\n0\n0\n");
}

/// Duration 字段访问与参与算术。
#[test]
fn duration_field_arith() {
    let out = run(
        r#"
fn main() {
    let d = Duration { micros: 1000 };
    // 字段读取
    println(d.micros == 1000);       // true
    // 秒 + 微秒混合运算（1000 微秒 = 0 秒 + 1000 微秒）
    let total = d.secs() * 1000000 + d.micros();
    println(total);                  // 1000
    // 比较
    let d2 = Duration { micros: 2000 };
    println(d2.micros > d.micros);   // true
    println(d2.micros - d.micros);   // 1000
}
"#,
    );
    assert_eq!(out, "true\n1000\ntrue\n1000\n");
}

/// Instant::now / elapsed：CPU 时钟差恒 >= 0，且计算后时间流逝（宽松断言）。
#[test]
fn instant_now_elapsed() {
    let out = run(
        r#"
fn main() {
    let i = Instant::now();
    // 消耗 CPU 时间
    let mut x = 0;
    let mut k = 0;
    while k < 200000 {
        x = x + k;
        k = k + 1;
    }
    let e = i.elapsed();
    println(e.micros() >= 0);        // true（时钟差恒非负）
    // 计算确实执行了
    println(x > 0);                  // true
    // 两次 now 的时钟值差非负（第二次不早于第一次）
    let a = Instant::now();
    let b = Instant::now();
    println(b.start >= a.start);     // true
}
"#,
    );
    assert_eq!(out, "true\ntrue\ntrue\n");
}

/// S2c `future::timeout` 成功路径：时限内 `Ready` 返回 `Ok(值)`。
#[test]
fn timeout_ok_returns_value() {
    let out = run(
        r#"
struct MyFut { state: i64 }
impl Future for MyFut {
    type Output = i64;
    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output> {
        self.state += 1;
        if self.state >= 3 {
            Poll::Ready(self.state)
        } else {
            Poll::Pending
        }
    }
}
fn main() {
    let mut f = MyFut { state: 0 };
    match timeout(Duration::milliseconds(100), &mut f) {
        Result::Ok(v) => println(v),    // 3
        Result::Err(_) => println(-1),
    }
}
"#,
    );
    assert_eq!(out, "3\n");
}

/// S2c/W4 `future::timeout` 超时路径：时限内未 `Ready` 返回 `Err(TimeoutError)`。
#[test]
fn timeout_expired_returns_err() {
    let out = run(
        r#"
struct NeverFut { dummy: i64 }
impl Future for NeverFut {
    type Output = i64;
    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output> {
        Poll::Pending
    }
}
fn main() {
    let mut f = NeverFut { dummy: 0 };
    match timeout(Duration::milliseconds(50), &mut f) {
        Result::Ok(v) => println(v),
        Result::Err(e) => println(e.message()),
    }
}
"#,
    );
    assert_eq!(out, "future timed out\n");
}
