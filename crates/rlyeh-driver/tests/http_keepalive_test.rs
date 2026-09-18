//! Y3（2026-08）HTTP 连接复用（keep-alive）集成测试。
//!
//! 免外网策略：Rust `std::net::TcpListener` 起本地 mock 服务器（多连接 +
//! 每连接多请求，每连接一线程），Rlyeh 程序经 `HttpClient` 实例方法访问，验证：
//! - `http_keepalive_reuse`：同一实例连续 GET 同 host → 复用同一连接
//!   （服务器端断言同一连接收到 2 个请求），响应按 Content-Length 精确读取
//!   （服务器不关闭连接）；
//! - `http_keepalive_two_clients`：两个独立实例 → 各建各的连接；
//! - `http_keepalive_stale_retry`：复用连接失效（服务器响应一次后关闭，模拟
//!   keep-alive 超时）→ 自动丢弃并新建连接重试一次；
//! - `http_keepalive_post`：同一实例连续 POST → 复用 + body 逐字节正确；
//! - `http_keepalive_no_content_length`：响应无 Content-Length → 回退 EOF 终止
//!   （兼容 Connection: close 服务器）。
//!
//! 需要系统 clang（与 net_http_test.rs 相同）。

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-ka-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入标准库），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("HTTP keep-alive 测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 读一个 HTTP 请求：请求行 + 头部（到 `\r\n\r\n`），再按 Content-Length 读 body。
/// 流 EOF 时返回空 head。返回 (头部原文, body 原文)。
fn read_request(stream: &mut std::net::TcpStream) -> (String, String) {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    let mut seen = 0u8; // \r\n\r\n 状态机
    while seen < 4 {
        if stream.read(&mut byte).unwrap_or(0) == 0 {
            break;
        }
        let b = byte[0];
        buf.push(b);
        if (seen == 0 && b == b'\r')
            || (seen == 1 && b == b'\n')
            || (seen == 2 && b == b'\r')
        {
            seen += 1;
        } else if seen == 3 && b == b'\n' {
            seen = 4;
        } else {
            seen = 0;
        }
    }
    let head = String::from_utf8_lossy(&buf).to_string();
    let mut body = String::new();
    if let Some(cl) = head.lines().find_map(|l| {
        l.to_ascii_lowercase()
            .strip_prefix("content-length:")
            .map(|s| s.trim().to_string())
    }) {
        if let Ok(n) = cl.parse::<usize>() {
            let mut payload = vec![0u8; n];
            if n > 0 && stream.read_exact(&mut payload).is_ok() {
                body = String::from_utf8_lossy(&payload).to_string();
            }
        }
    }
    (head, body)
}

/// 多连接 keep-alive mock 服务器：accept 主循环 + 每连接一线程，逐连接循环读
/// 请求（read_request）。handler(连接序, 请求序, 头, body) -> (响应字节, 是否保持连接)。
/// 保持=true 继续读下一请求；false 关闭连接（模拟 keep-alive 超时/主动 close）。
/// 客户端进程退出（EOF）后连接自然结束。
/// 返回 (端口, 各连接收到请求数：conn_idx → 请求数)。
fn mock_keepalive_server(
    handler: impl Fn(usize, usize, String, String) -> (String, bool) + Send + Sync + 'static,
) -> (u16, Arc<Mutex<HashMap<usize, usize>>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("绑定 mock 端口失败");
    let port = listener.local_addr().expect("读取 mock 端口失败").port();
    let counts: Arc<Mutex<HashMap<usize, usize>>> = Arc::new(Mutex::new(HashMap::new()));
    let handler: Arc<dyn Fn(usize, usize, String, String) -> (String, bool) + Send + Sync> =
        Arc::new(handler);
    let counts_c = counts.clone();
    std::thread::spawn(move || {
        for conn_idx in 0..16 {
            let (mut stream, _) = match listener.accept() {
                Ok(s) => s,
                Err(_) => break,
            };
            let c2 = counts_c.clone();
            let h = handler.clone();
            std::thread::spawn(move || {
                let mut n = 0usize;
                loop {
                    let (head, body) = read_request(&mut stream);
                    if head.is_empty() {
                        break; // EOF：客户端关闭
                    }
                    let (resp, keep) = h(conn_idx, n, head, body);
                    n += 1;
                    if stream.write_all(resp.as_bytes()).is_err() {
                        break;
                    }
                    if !keep {
                        break;
                    }
                }
                c2.lock().unwrap().insert(conn_idx, n);
            });
        }
    });
    (port, counts)
}

/// 轮询等待服务器端收集满 expect 个请求（客户端进程退出后连接线程才记账）。
fn wait_conns(
    counts: &Arc<Mutex<HashMap<usize, usize>>>,
    expect: usize,
) -> HashMap<usize, usize> {
    let start = Instant::now();
    loop {
        let m = counts.lock().unwrap().clone();
        let total: usize = m.values().sum();
        if total >= expect || start.elapsed() > Duration::from_secs(5) {
            return m;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// 同一实例连续 GET 同 host → 复用同一连接（服务器端 1 连接 2 请求）。
#[test]
fn http_keepalive_reuse() {
    let paths = Arc::new(Mutex::new(Vec::new()));
    let p2 = paths.clone();
    let (port, counts) = mock_keepalive_server(move |_ci, ri, head, _body| {
        p2.lock()
            .unwrap()
            .push(head.lines().next().unwrap_or("").to_string());
        let body = if ri == 0 { "one" } else { "two" };
        (
            format!("HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: keep-alive\r\n\r\n{body}"),
            true,
        )
    });
    let src = format!(
        r#"
fn main() {{
    let mut c = HttpClient::new();
    match c.get(String::from("http://127.0.0.1:{port}/a")) {{
        Result::Ok(r) => {{ println(r.status()); println(r.text()); }},
        Result::Err(e) => println(-1),
    }}
    match c.get(String::from("http://127.0.0.1:{port}/b")) {{
        Result::Ok(r) => {{ println(r.status()); println(r.text()); }},
        Result::Err(e) => println(-1),
    }}
}}
"#
    );
    let out = run(&src);
    assert_eq!(out, "200\none\n200\ntwo\n");
    let c = wait_conns(&counts, 2);
    assert_eq!(c.len(), 1, "应复用同一连接: {c:?}");
    assert_eq!(c.get(&0), Some(&2), "同一连接应收到 2 个请求: {c:?}");
    let ps = paths.lock().unwrap().clone();
    assert_eq!(ps.len(), 2, "服务器应收到 2 个请求: {ps:?}");
    assert!(ps[0].starts_with("GET /a HTTP/1.1"), "请求行不符: {ps:?}");
    assert!(ps[1].starts_with("GET /b HTTP/1.1"), "请求行不符: {ps:?}");
}

/// 两个独立实例 → 各建各的连接（服务器端 2 连接各 1 请求）。
#[test]
fn http_keepalive_two_clients() {
    let (port, counts) = mock_keepalive_server(|_ci, _ri, _head, _body| {
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: keep-alive\r\n\r\none".to_string(),
            true,
        )
    });
    let src = format!(
        r#"
fn main() {{
    let mut c1 = HttpClient::new();
    match c1.get(String::from("http://127.0.0.1:{port}/a")) {{
        Result::Ok(r) => println(r.status()),
        Result::Err(e) => println(-1),
    }}
    let mut c2 = HttpClient::new();
    match c2.get(String::from("http://127.0.0.1:{port}/a")) {{
        Result::Ok(r) => println(r.status()),
        Result::Err(e) => println(-1),
    }}
}}
"#
    );
    let out = run(&src);
    assert_eq!(out, "200\n200\n");
    let c = wait_conns(&counts, 2);
    assert_eq!(c.len(), 2, "两个实例应各建各的连接: {c:?}");
    assert_eq!(c.get(&0), Some(&1));
    assert_eq!(c.get(&1), Some(&1));
}

/// 复用连接失效（服务器响应一次后关闭）→ 自动丢弃并新建连接重试一次。
#[test]
fn http_keepalive_stale_retry() {
    let (port, counts) = mock_keepalive_server(|ci, ri, _head, _body| {
        if ci == 0 {
            // 第一连接：响应后关闭（模拟服务器 keep-alive 超时/主动 close）
            assert_eq!(ri, 0);
            ("HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\none".to_string(), false)
        } else {
            // 第二连接：正常响应（客户端重连成功）
            ("HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\ntwo".to_string(), true)
        }
    });
    let src = format!(
        r#"
fn main() {{
    let mut c = HttpClient::new();
    match c.get(String::from("http://127.0.0.1:{port}/a")) {{
        Result::Ok(r) => {{ println(r.status()); println(r.text()); }},
        Result::Err(e) => println(-1),
    }}
    match c.get(String::from("http://127.0.0.1:{port}/b")) {{
        Result::Ok(r) => {{ println(r.status()); println(r.text()); }},
        Result::Err(e) => println(-1),
    }}
}}
"#
    );
    let out = run(&src);
    assert_eq!(out, "200\none\n200\ntwo\n");
    let c = wait_conns(&counts, 2);
    assert_eq!(c.len(), 2, "第一连接失效后应重连到第二连接: {c:?}");
    assert_eq!(c.get(&0), Some(&1));
    assert_eq!(c.get(&1), Some(&1));
}

/// 同一实例连续 POST → 复用连接 + body 逐字节正确（服务器端回显校验）。
#[test]
fn http_keepalive_post() {
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let b2 = bodies.clone();
    let (port, counts) = mock_keepalive_server(move |_ci, ri, _head, body| {
        b2.lock().unwrap().push(body.clone());
        let expect = if ri == 0 { "rlyeh-ka1" } else { "rlyeh-ka2" };
        assert_eq!(body, expect, "服务器端收到的 body 不符");
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: keep-alive\r\n\r\ndone".to_string(),
            true,
        )
    });
    let src = format!(
        r#"
fn main() {{
    let mut c = HttpClient::new();
    match c.post(String::from("http://127.0.0.1:{port}/s1"), String::from("rlyeh-ka1")) {{
        Result::Ok(r) => {{ println(r.status()); println(r.text()); }},
        Result::Err(e) => println(-1),
    }}
    match c.post(String::from("http://127.0.0.1:{port}/s2"), String::from("rlyeh-ka2")) {{
        Result::Ok(r) => {{ println(r.status()); println(r.text()); }},
        Result::Err(e) => println(-1),
    }}
}}
"#
    );
    let out = run(&src);
    assert_eq!(out, "200\ndone\n200\ndone\n");
    let c = wait_conns(&counts, 2);
    assert_eq!(c.len(), 1, "POST 应复用同一连接: {c:?}");
    assert_eq!(c.get(&0), Some(&2));
    let bs = bodies.lock().unwrap().clone();
    assert_eq!(bs, vec!["rlyeh-ka1".to_string(), "rlyeh-ka2".to_string()]);
}

/// 响应无 Content-Length → 回退 EOF 终止（兼容 Connection: close 服务器）。
#[test]
fn http_keepalive_no_content_length() {
    let (port, counts) = mock_keepalive_server(|_ci, _ri, _head, _body| {
        // 无 Content-Length：客户端必须回退 EOF 读取才能拿到 body
        ("HTTP/1.1 200 OK\r\n\r\nfallback-body".to_string(), false)
    });
    let src = format!(
        r#"
fn main() {{
    let mut c = HttpClient::new();
    match c.get(String::from("http://127.0.0.1:{port}/nocl")) {{
        Result::Ok(r) => {{ println(r.status()); println(r.text()); }},
        Result::Err(e) => println(-1),
    }}
}}
"#
    );
    let out = run(&src);
    assert_eq!(out, "200\nfallback-body\n");
    let c = wait_conns(&counts, 1);
    assert_eq!(c.get(&0), Some(&1));
}
