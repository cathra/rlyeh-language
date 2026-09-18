//! R 阶段（2026-08）NIO（非阻塞模式 + Poller 事件轮询）+ sendfile 零拷贝
//! 集成测试（自动注入 `rlyeh-std/rlyeh/`）。
//!
//! 免外网策略：本地 `socketpair_stream`（同一进程内全双工 fd）+ Rust
//! `std::net::TcpListener` 起本地 mock 对端（随机端口 + 单连接线程）。
//!
//! 覆盖：
//! - `set_nonblocking`/`is_nonblocking`：fcntl O_NONBLOCK 位设置/查询往返
//! - `Poller`：register → 对端写入触发 POLLIN 事件（token/readable 断言）、
//!   reregister 改关注方向（POLLOUT）、deregister 后轮询为空、
//!   重复 register（AlreadyExists）/ 未注册 deregister（NotFound）、
//!   `Poller::new()` 构造
//! - `sendfile` 自由函数：File(fileno) → TCP 连接，指定 count 发送
//! - `File::sendfile_to`：offset 起至 EOF 零拷贝传输，服务端读回内容
//!
//! 需要系统 clang（与 net_http_test.rs / net_socket_test.rs 相同）。

use std::io::Read;
use std::net::TcpListener;
use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-nio-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入标准库），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("NIO 模块测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 起本地 mock TCP 对端：accept 后精确读取 n 字节，返回内容。
/// Rlyeh 侧经 `TcpStream::connect` 连接 `{port}`。
fn tcp_server_read(n: usize) -> (u16, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定 mock 端口失败");
    let port = listener.local_addr().expect("读取 mock 端口失败").port();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("mock 服务端 accept 失败");
        let mut buf = vec![0u8; n];
        stream.read_exact(&mut buf).expect("mock 服务端读取失败");
        String::from_utf8_lossy(&buf).to_string()
    });
    (port, handle)
}

/// `Poller::new()` 构造成功（Ok 分支返回空注册表）。
#[test]
fn poller_new() {
    let out = run(
        r#"
fn main() {
    match Poller::new() {
        Ok(p) => println(p.fds.len()),
        Err(_) => println(-1),
    }
}
"#,
    );
    assert_eq!(out, "0\n");
}

/// R2：非阻塞模式设置/查询往返（socketpair 两端 fd）。
#[test]
fn nonblocking_flag() {
    let src = r#"
fn main() {
    let fds = socketpair_stream();
    let a = fd_at(fds, 0);
    let b = fd_at(fds, 1);
    match set_nonblocking(a, true) {
        Ok(n1) => println(n1),
        Err(_) => println(-1),
    }
    match is_nonblocking(a) {
        Ok(b1) => {
            if b1 {
                println(1)
            } else {
                println(0)
            }
        },
        Err(_) => println(-1),
    }
    match set_nonblocking(a, false) {
        Ok(n2) => println(n2),
        Err(_) => println(-1),
    }
    match is_nonblocking(a) {
        Ok(b2) => {
            if b2 {
                println(1)
            } else {
                println(0)
            }
        },
        Err(_) => println(-1),
    }
    let _ = unsafe { close(a) };
    let _ = unsafe { close(b) };
}
"#;
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("NIO 模块测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    println!("OUT: {out:?}");
    assert_eq!(out, "1\n1\n1\n0\n");
}

/// R1b：register 关注 Readable → 对端写入 → poll 返回就绪事件
/// （token 正确、is_readable、内容可 recv 读回）。
#[test]
fn poller_readable() {
    let out = run(
        r#"
fn main() {
    let fds = socketpair_stream();
    let a = fd_at(fds, 0);
    let b = fd_at(fds, 1);
    let mut p = Poller { fds: Vec::new(), events: Vec::new(), tokens: Vec::new(), kq: -1 };
    match p.register(a, 7, Interest::Readable) {
        Ok(n) => println(n),
        Err(_) => println(-1),
    }
    // 对端写入后才 poll（POLLIN 就绪）
    let _ = send_all(b, String::from("hi"));
    match p.poll(2000) {
        Ok(evs) => {
            println(evs.len());
            let e0 = evs[0];
            println(e0.token);
            println(e0.is_readable());
        },
        Err(_) => println(-1),
    }
    match recv_some(a, 8) {
        Ok(r) => println(r),
        Err(_) => println(-1),
    }
    let _ = unsafe { close(a) };
    let _ = unsafe { close(b) };
}
"#,
    );
    assert_eq!(out, "1\n1\n7\ntrue\nhi\n");
}

/// R1b：reregister 改关注方向（Readable → Writable，POLLOUT 立即可用），
/// 事件含正确 token 与 writable 标志；deregister 后轮询立即为空。
#[test]
fn poller_writable_reregister() {
    let out = run(
        r#"
fn main() {
    let fds = socketpair_stream();
    let a = fd_at(fds, 0);
    let b = fd_at(fds, 1);
    let mut p = Poller { fds: Vec::new(), events: Vec::new(), tokens: Vec::new(), kq: -1 };
    let _ = p.register(a, 1, Interest::Readable);
    match p.reregister(a, 9, Interest::Writable) {
        Ok(n) => println(n),
        Err(_) => println(-1),
    }
    match p.poll(2000) {
        Ok(evs) => {
            println(evs.len());
            let e0 = evs[0];
            println(e0.token);
            println(e0.is_writable());
        },
        Err(_) => println(-1),
    }
    match p.deregister(a) {
        Ok(n) => println(n),
        Err(_) => println(-1),
    }
    match p.poll(100) {
        Ok(evs) => println(evs.len()),
        Err(_) => println(-1),
    }
    let _ = unsafe { close(a) };
    let _ = unsafe { close(b) };
}
"#,
    );
    assert_eq!(out, "1\n1\n9\ntrue\n1\n0\n");
}

/// R1b：重复 register → AlreadyExists（Err）；未注册 fd deregister → NotFound（Err）。
#[test]
fn poller_errors() {
    let out = run(
        r#"
fn main() {
    let fds = socketpair_stream();
    let a = fd_at(fds, 0);
    let b = fd_at(fds, 1);
    let mut p = Poller { fds: Vec::new(), events: Vec::new(), tokens: Vec::new(), kq: -1 };
    let _ = p.register(a, 1, Interest::Readable);
    match p.register(a, 2, Interest::Readable) {
        Ok(n) => println(0),
        Err(_) => println(1),
    }
    match p.deregister(b) {
        Ok(n) => println(0),
        Err(_) => println(2),
    }
    let _ = unsafe { close(a) };
    let _ = unsafe { close(b) };
}
"#,
    );
    assert_eq!(out, "1\n2\n");
}

/// P1（2026-08-28）：kqueue 分派路径——`Poller::new()` 在 macOS/BSD 建真 kqueue，
/// register → 对端写入 → poll 走 kevent 返回就绪事件（token/readable）。
/// 仅当 `Poller::new()` 走 kqueue 分支（macOS/BSD 码 2/4）时验证；
/// 其他平台（Linux/Windows）回退 poll(2)，同样应就绪。
#[test]
fn poller_kqueue_dispatch() {
    let out = run(
        r#"
fn main() {
    match Poller::new() {
        Ok(p) => {
            let mut pp = p;
            let fds = socketpair_stream();
            let a = fd_at(fds, 0);
            let b = fd_at(fds, 1);
            match pp.register(a, 42, Interest::Readable) {
                Ok(n) => println(n),
                Err(_) => println(-1),
            }
            let _ = send_all(b, String::from("hello"));
            match pp.poll(2000) {
                Ok(evs) => {
                    println(evs.len());
                    let e0 = evs[0];
                    println(e0.token);
                    println(e0.is_readable());
                },
                Err(_) => println(-1),
            }
            let _ = unsafe { close(a) };
            let _ = unsafe { close(b) };
        },
        Err(_) => println(-1),
    }
}
"#,
    );
    // new() 走 kqueue（macOS/BSD）或 poll 回退（其他）：register=1, 就绪=1, token=42, readable=true
    assert_eq!(out, "1\n1\n42\ntrue\n");
}

/// R3：sendfile 自由函数——File(fileno) → TCP 连接，offset=0 count=13 发送，
/// mock 对端读回完整内容。
#[test]
fn sendfile_free_fn() {
    let (port, handle) = tcp_server_read(13);
    let src = format!(
        r#"
fn main() {{
    match File::create(String::from("/tmp/rlyeh-sendfile-src.txt")) {{
        Ok(f) => {{
            let mut ff = f;
            match ff.write(String::from("rlyeh-sendfile")) {{
                Ok(n) => println(n),
                Err(_) => println(-1),
            }}
            match ff.flush() {{
                Ok(n) => println(n),
                Err(_) => println(-1),
            }}
            let _ = ff.close();
        }},
        Err(_) => println(-1),
    }}
    match File::open(String::from("/tmp/rlyeh-sendfile-src.txt")) {{
        Ok(f) => {{
            let file_fd = unsafe {{ fileno(f.handle) }};
            match TcpStream::connect(SocketAddr {{ ip: String::from("127.0.0.1"), port: {port} }}) {{
                Ok(s) => {{
                    match sendfile(s.fd, file_fd, 0, 13) {{
                        Ok(n) => println(n),
                        Err(_) => println(-1),
                    }}
                    match s.shutdown(Shutdown::Write) {{
                        Ok(n) => println(1),
                        Err(_) => println(-1),
                    }}
                }},
                Err(_) => println(-1),
            }}
            let _ = f.close();
        }},
        Err(_) => println(-1),
    }}
}}
"#
    );
    let out = run(&src);
    // write 返回 14（"rlyeh-sendfile" 14 字节）；sendfile count=13 仅传 13 字节
    assert_eq!(out, "14\n1\n13\n1\n");
    // mock 对端精确读 13 字节（count=13 的部分传输语义）
    assert_eq!(handle.join().expect("mock 线程失败"), "rlyeh-sendfil");
}

/// R3：`File::sendfile_to(sock_fd, offset)`——offset 起至 EOF 零拷贝传输，
/// 返回实际字节数；mock 对端读回偏移后内容。
///
/// 注意：sendfile(2) 要求 in_fd 为可读 fd（O_RDONLY），
/// 故先 create+write+flush+close 落盘，再以只读模式 reopen 取句柄。
#[test]
fn sendfile_to_method() {
    let (port, handle) = tcp_server_read(12);
    let src = format!(
        r#"
fn main() {{
    match File::create(String::from("/tmp/rlyeh-sendfile-method.txt")) {{
        Ok(f) => {{
            let mut ff = f;
            match ff.write(String::from("0123456789abcdef")) {{
                Ok(n) => println(n),
                Err(_) => println(-1),
            }}
            match ff.flush() {{
                Ok(n) => println(n),
                Err(_) => println(-1),
            }}
            let _ = ff.close();
        }},
        Err(_) => println(-1),
    }}
    match File::open(String::from("/tmp/rlyeh-sendfile-method.txt")) {{
        Ok(f) => {{
            match TcpStream::connect(SocketAddr {{ ip: String::from("127.0.0.1"), port: {port} }}) {{
                Ok(s) => {{
                    match f.sendfile_to(s.fd, 4) {{
                        Ok(n) => println(n),
                        Err(_) => println(-1),
                    }}
                    match s.shutdown(Shutdown::Write) {{
                        Ok(n) => println(1),
                        Err(_) => println(-1),
                    }}
                }},
                Err(_) => println(-1),
            }}
            let _ = f.close();
        }},
        Err(_) => println(-1),
    }}
}}
"#
    );
    let out = run(&src);
    assert_eq!(out, "16\n1\n12\n1\n");
    assert_eq!(handle.join().expect("mock 线程失败"), "456789abcdef");
}
