//! O3（2026-08）HTTP 客户端集成测试（`HttpClient::get/post` + `Response`）。
//!
//! 免外网策略：本文件用 Rust `std::net::TcpListener` 起本地 mock HTTP 服务器
//! （随机端口 + 单请求线程），Rlyeh 程序经 `HttpClient::get/post` 访问并断言
//! 状态码 / body 文本 / `json::parse::<T>` 反序列化（L2）。
//!
//! 覆盖：`get`（200 + JSON body）、`post`（Content-Length + 服务器端 echo）、
//! `404` 状态解析、async 退化（S3b）。Y3 起 `get/post` 为实例方法
//! （`let mut c = HttpClient::new(); c.get(url)`）；连接复用用例见
//! `http_keepalive_test.rs`。
//!
//! 需要系统 clang（与 std_test.rs / net_socket_test.rs 相同）。

use std::io::{Read, Write};
use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-http-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.rl），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("HTTP 模块测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 读一个 HTTP 请求：请求行 + 头部（到 `\r\n\r\n`），再按 Content-Length 读 body。
/// 返回 (头部原文, body 原文)。
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
    // 按 Content-Length 读 body
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

/// 起一个单请求 mock HTTP 服务器，`respond(头部, body)` 生成响应字节。
fn mock_server(respond: impl FnOnce(String, String) -> String + Send + 'static) -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("绑定 mock 端口失败");
    let port = listener.local_addr().expect("读取 mock 端口失败").port();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let (head, body) = read_request(&mut stream);
            let resp = respond(head, body);
            let _ = stream.write_all(resp.as_bytes());
            // 依赖 TCP 关闭的 EOF 终止 Rlyeh 端 read_all（Connection: close 语义）
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
    });
    port
}

/// `HttpClient::get`：200 + JSON body + `json::parse::<i64>` 反序列化。
#[test]
fn http_get_json() {
    let port = mock_server(|head, _body| {
        assert!(head.starts_with("GET /api/v1 HTTP/1.1"), "请求行不符: {head:?}");
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n42".to_string()
    });
    let src = format!(
        r#"
fn main() {{
    let mut c = HttpClient::new();
    println(c._unit == 0);   // 占位字段（O3a 构造）
    match c.get(String::from("http://127.0.0.1:{port}/api/v1")) {{
        Result::Ok(r) => {{
            println(r.status());
            println(r.text());
            println(json::parse::<i64>(r.text()));
        }},
        Result::Err(e) => println(-1),
    }}
}}
"#
    );
    let out = run(&src);
    assert_eq!(out, "true\n200\n42\n42\n");
}

/// `HttpClient::post`：服务器端回显收到的 body，验证 Content-Length 与字节内容。
#[test]
fn http_post_echo() {
    let port = mock_server(|head, body| {
        assert_eq!(body, "rlyeh-post", "服务器端收到的 body 不符: {body:?}");
        assert!(head.starts_with("POST /submit HTTP/1.1"), "请求行不符: {head:?}");
        "HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\ndone".to_string()
    });
    let src = format!(
        r#"
fn main() {{
    let mut c = HttpClient::new();
    match c.post(String::from("http://127.0.0.1:{port}/submit"), String::from("rlyeh-post")) {{
        Result::Ok(r) => {{
            println(r.status());
            println(r.text());
        }},
        Result::Err(e) => println(-1),
    }}
}}
"#
    );
    let out = run(&src);
    assert_eq!(out, "200\ndone\n");
}

/// 404 状态解析。
#[test]
fn http_not_found_status() {
    let port = mock_server(|_head, _body| {
        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
    });
    let src = format!(
        r#"
fn main() {{
    let mut c = HttpClient::new();
    match c.get(String::from("http://127.0.0.1:{port}/missing")) {{
        Result::Ok(r) => println(r.status()),
        Result::Err(e) => println(-1),
    }}
}}
"#
    );
    let out = run(&src);
    assert_eq!(out, "404\n");
}

/// W5：`HttpClient::get_async` 真异步——返回 `GetAsync` future，`block_on` 驱动
/// 读响应（connect/写同步，读经 wait_fd 挂起），返回 `Response`。
#[test]
fn http_get_async() {
    let port = mock_server(|head, _body| {
        assert!(head.starts_with("GET /async HTTP/1.1"), "请求行不符: {head:?}");
        "HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nasync1".to_string()
    });
    let src = format!(
        r#"
fn main() {{
    let mut c = HttpClient::new();
    let mut g = c.get_async(String::from("http://127.0.0.1:{port}/async"));
    let r = block_on(&mut g);
    println(r.status());
    println(r.text());
}}
"#
    );
    let out = run(&src);
    assert_eq!(out, "200\nasync1\n");
}

/// W5：`HttpClient::post_async` 真异步——返回 `GetAsync` future（POST 带 body），
/// `block_on` 驱动读响应，返回 `Response`。
#[test]
fn http_post_async() {
    let port = mock_server(|head, body| {
        assert_eq!(body, "rlyeh-post-async", "服务器端收到的 body 不符: {body:?}");
        assert!(head.starts_with("POST /async-submit HTTP/1.1"), "请求行不符: {head:?}");
        "HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\ndone".to_string()
    });
    let src = format!(
        r#"
fn main() {{
    let mut c = HttpClient::new();
    let mut g = c.post_async(String::from("http://127.0.0.1:{port}/async-submit"), String::from("rlyeh-post-async"));
    let r = block_on(&mut g);
    println(r.status());
    println(r.text());
}}
"#
    );
    let out = run(&src);
    assert_eq!(out, "200\ndone\n");
}
