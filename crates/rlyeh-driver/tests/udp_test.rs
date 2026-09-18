//! Y7（2026-08）UDP 数据报集成测试。
//!
//! 免外网策略：全部走 `127.0.0.1` loopback，socket 均 `bind(0)`（内核分配
//! 端口，避免并行测试端口冲突），经 `local_addr().port()` 取实际端口互发。
//! 覆盖：
//! - `udp_loopback_self_echo`：自回环——send_to 自身 + recv_from 回显，
//!   源端口回填 == 本地端口；
//! - `udp_two_sockets_peer_roundtrip`：两 socket 互发，源端口互相正确
//!   （A→B、B→A 的 `from.port()` 验证）；
//! - `udp_multi_datagrams_order`：连续 3 包，逐包按序接收 + 内容 + 源端口。
//!
//! 需要系统 clang（与 net_http_test.rs 相同）。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-udp-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入标准库），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("UDP 测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 自回环：bind(0) → send_to 自身 → recv_from 回显，源端口回填 == 本地端口。
#[test]
fn udp_loopback_self_echo() {
    let src = r#"
fn main() {
    let addr = SocketAddr::new(String::from("127.0.0.1"), 0);
    let s = match UdpSocket::bind(addr) {
        Result::Ok(x) => x,
        Result::Err(e) => return,
    };
    let lp = s.local_addr().port();
    let target = SocketAddr::new(String::from("127.0.0.1"), lp);
    match s.send_to(String::from("hello-udp"), target) {
        Result::Ok(n) => println(n),
        Result::Err(e) => println(-1),
    }
    match s.recv_from(128) {
        Result::Ok(p) => {
            println(p.data);
            println(p.from.port() == lp);
        }
        Result::Err(e) => println(-2),
    }
}
"#;
    let out = run(src);
    assert_eq!(out, "9\nhello-udp\ntrue\n");
}

/// 两 socket 互发：A→B、B→A，源端口互相正确。
#[test]
fn udp_two_sockets_peer_roundtrip() {
    let src = r#"
fn main() {
    let a = match UdpSocket::bind(SocketAddr::new(String::from("127.0.0.1"), 0)) {
        Result::Ok(x) => x,
        Result::Err(e) => return,
    };
    let b = match UdpSocket::bind(SocketAddr::new(String::from("127.0.0.1"), 0)) {
        Result::Ok(x) => x,
        Result::Err(e) => return,
    };
    let ap = a.local_addr().port();
    let bp = b.local_addr().port();
    match a.send_to(String::from("a-to-b"), SocketAddr::new(String::from("127.0.0.1"), bp)) {
        Result::Ok(n) => println(n),
        Result::Err(e) => println(-1),
    }
    match b.recv_from(128) {
        Result::Ok(p) => {
            println(p.data);
            println(p.from.port() == ap);
        }
        Result::Err(e) => println(-2),
    }
    match b.send_to(String::from("b-to-a"), SocketAddr::new(String::from("127.0.0.1"), ap)) {
        Result::Ok(n) => println(n),
        Result::Err(e) => println(-1),
    }
    match a.recv_from(128) {
        Result::Ok(p) => {
            println(p.data);
            println(p.from.port() == bp);
        }
        Result::Err(e) => println(-2),
    }
}
"#;
    let out = run(src);
    assert_eq!(out, "6\na-to-b\ntrue\n6\nb-to-a\ntrue\n");
}

/// 连续 3 包：按序接收 + 内容逐包校验 + 源端口一致。
#[test]
fn udp_multi_datagrams_order() {
    let src = r#"
fn main() {
    let a = match UdpSocket::bind(SocketAddr::new(String::from("127.0.0.1"), 0)) {
        Result::Ok(x) => x,
        Result::Err(e) => return,
    };
    let b = match UdpSocket::bind(SocketAddr::new(String::from("127.0.0.1"), 0)) {
        Result::Ok(x) => x,
        Result::Err(e) => return,
    };
    let ap = a.local_addr().port();
    let bp = b.local_addr().port();
    match a.send_to(String::from("pkt-1"), SocketAddr::new(String::from("127.0.0.1"), bp)) {
        Result::Ok(n) => println(n),
        Result::Err(e) => println(-1),
    }
    match a.send_to(String::from("pkt-2"), SocketAddr::new(String::from("127.0.0.1"), bp)) {
        Result::Ok(n) => println(n),
        Result::Err(e) => println(-1),
    }
    match a.send_to(String::from("pkt-3"), SocketAddr::new(String::from("127.0.0.1"), bp)) {
        Result::Ok(n) => println(n),
        Result::Err(e) => println(-1),
    }
    let mut i = 0;
    while i < 3 {
        match b.recv_from(128) {
            Result::Ok(p) => {
                println(p.data);
                println(p.from.port() == ap);
            }
            Result::Err(e) => println(-2),
        }
        i = i + 1;
    }
}
"#;
    let out = run(src);
    assert_eq!(out, "5\n5\n5\npkt-1\ntrue\npkt-2\ntrue\npkt-3\ntrue\n");
}
