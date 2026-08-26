//! W3（2026-08-25）：事件驱动 executor 集成测试（第一步：定时器唤醒）。
//!
//! `Context` 升级携带 `deadline` 槽，`block_on` 在 future `Pending` 且经
//! `cx.deadline` 请求唤醒时刻时休眠到该时刻再轮询（不忙等）；`future::sleep`
//! 为定时器 future。验证：sleep 期间墙钟推进而 CPU 时间远小于墙钟（进程真正
//! 休眠，非忙等）。
//!
//! 需要系统 clang（与 thread_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-exec-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.rl），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("W3 executor 测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 事件驱动 sleep：async fn 内 await `future::sleep(20ms)`，返回 42。
/// 输出三行：返回值 / 墙钟 elapsed(us) / CPU 时间(us)。断言墙钟 ≥ 20000us
/// 且 CPU 时间 < 墙钟的一半（进程真正休眠，非忙等）。
#[test]
fn async_sleep_is_event_driven_not_busy() {
    let out = run(
        r#"
async fn g() -> i64 {
    let s: future::Sleep = future::sleep(Duration::milliseconds(20));
    s.await;
    42
}
fn main() {
    let w0 = __rlyeh_clock_monotonic();
    let c0 = clock();
    let mut f = g();
    let v = block_on(&mut f);
    let w1 = __rlyeh_clock_monotonic();
    let c1 = clock();
    println(v);
    println(w1 - w0);
    println(c1 - c0);
}
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3, "输出应为 3 行: {out}");
    assert_eq!(lines[0], "42");
    let wall: i64 = lines[1].trim().parse().expect("墙钟 elapsed 应可解析");
    let cpu: i64 = lines[2].trim().parse().expect("CPU 时间应可解析");
    assert!(wall >= 20_000, "墙钟 elapsed 应 ≥ 20ms（sleep 20ms），实际 {wall}us");
    assert!(
        cpu < wall / 2,
        "CPU 时间应远小于墙钟（事件驱动休眠非忙等），CPU {cpu}us vs 墙钟 {wall}us"
    );
}

/// 手写 future 也可经 `cx.deadline` 请求定时唤醒（写 Context 的 deadline 槽）。
#[test]
fn manual_future_can_request_deadline() {
    let out = run(
        r#"
struct Timer { target: i64, n: i64 }
impl Future for Timer {
    type Output = i64;
    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output> {
        self.n = self.n + 1;
        let t0 = __rlyeh_clock_monotonic();
        let now = if t0 >= 0 { t0 } else { clock() };
        if now >= self.target {
            Poll::Ready(self.n)
        } else {
            cx.deadline = self.target;
            Poll::Pending
        }
    }
}
fn main() {
    let w0 = __rlyeh_clock_monotonic();
    let c0 = clock();
    let mut f = Timer { target: w0 + 15_000, n: 0 };
    let v = block_on(&mut f);
    let w1 = __rlyeh_clock_monotonic();
    let c1 = clock();
    println(v);          // 2（poll 两次：1 次 Pending + 1 次 Ready）
    println(w1 - w0);    // 墙钟 ≥ 15000us
    println(c1 - c0);    // CPU << 墙钟
}
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "2");
    let wall: i64 = lines[1].trim().parse().unwrap();
    let cpu: i64 = lines[2].trim().parse().unwrap();
    assert!(wall >= 15_000, "墙钟应 ≥ 15ms，实际 {wall}us");
    assert!(cpu < wall / 2, "CPU 应 << 墙钟，CPU {cpu}us vs 墙钟 {wall}us");
}

/// fd 事件唤醒：`future::wait_fd(fd, Readable)` 等待 TcpListener 可读（有 pending
/// 连接）。线程延迟 30ms connect 触发就绪。验证：wait_fd 初始 Pending → block_on
/// 经 poll(2) 等待 fd 就绪（非忙等）→ 连接到来唤醒 Ready。墙钟 ≥ 30ms 而 CPU <<
/// 墙钟（进程休眠等 fd 事件）。固定高位端口，本机冲突概率极低。
#[test]
fn wait_fd_is_event_driven_not_busy() {
    let out = run(
        r#"
fn connecter() -> i64 {
    thread::sleep(Duration::milliseconds(30));
    match TcpStream::connect(SocketAddr::new(String::from("127.0.0.1"), 21346)) {
        Result::Ok(s) => 1,
        Result::Err(e) => 0,
    }
}
fn main() {
    let listener = match TcpListener::bind(SocketAddr::new(String::from("127.0.0.1"), 21346)) {
        Result::Ok(l) => l,
        Result::Err(e) => return,   // 端口被占用则跳过
    };
    let fd = listener.fd;
    let w0 = __rlyeh_clock_monotonic();
    let c0 = clock();
    match Thread::start(connecter) {
        Result::Ok(t) => {
            let wfd = future::wait_fd(fd, io::nio::Interest::Readable);
            let mut f = wfd;
            let v = block_on(&mut f);
            let w1 = __rlyeh_clock_monotonic();
            let c1 = clock();
            println(v);
            println(w1 - w0);
            println(c1 - c0);
        }
        Result::Err(e) => println(-99),
    }
}
"#,
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3, "输出应为 3 行: {out}");
    if lines[0].trim() == "-99" {
        return; // 线程启动失败，跳过
    }
    assert_eq!(lines[0], "0", "wait_fd 应在连接就绪时返回 0");
    let wall: i64 = lines[1].trim().parse().unwrap();
    let cpu: i64 = lines[2].trim().parse().unwrap();
    assert!(wall >= 30_000, "墙钟应 ≥ 30ms（等线程 connect），实际 {wall}us");
    assert!(
        cpu < wall / 2,
        "CPU 应 << 墙钟（poll 等 fd 事件非忙等），CPU {cpu}us vs 墙钟 {wall}us"
    );
}
