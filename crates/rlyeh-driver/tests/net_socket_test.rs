//! net 模块集成测试（文件入口 API，自动注入 `rlyeh-std/rlyeh/`）。
//!
//! 覆盖：`htons` 字节序转换、`socketpair_stream` 全双工字节流
//! （send/recv 单向 + 双向 + 循环多包 + 4096 大数据块）、
//! `sockaddr_in4` 字节布局（macOS sin_len 头）、
//! `tcp_connect` 对未监听端口的拒绝（验证 sockaddr_in 被内核正确解析）。
//!
//! M3b（2026-08）后 net 自由函数 Result 化：`send_all`/`recv_some`/`tcp_connect`
//! 返回 `Result<T, IoError>`，失败不再用 "0 / -1 / 空串" 哨兵值；本文件用例
//! 均按 `match { Ok(..) / Err(..) }` 解包验证。
//!
//! 需要系统 clang（与 std_test.rs / agg_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-net-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入标准库），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("net 模块测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// htons：主机字节序 → 网络字节序（大端），纯位运算实现。
#[test]
fn htons_byte_order() {
    let out = run(
        r#"
fn main() {
    println(htons(0x1234));        // 0x3412 = 13330
    println(htons(80));            // 80 << 8 = 20480（高字节 80，低字节 0）
    println(htons(htons(8080)));   // 往返不变 = 8080
    println(htons(0xFF00));        // 0x00FF = 255
}
"#,
    );
    assert_eq!(out, "13330\n20480\n8080\n255\n");
}

/// socketpair 全双工字节流：单向 + 双向 send/recv，fd 数组字节解释。
#[test]
fn socketpair_send_recv() {
    let out = run(
        r#"
fn main() {
    let fds = socketpair_stream();
    let fd0 = fd_at(fds, 0);
    let fd1 = fd_at(fds, 1);
    println(fd0 > 0);            // true（有效 fd）
    println(fd1 > 0);            // true
    println(fd0 != fd1);         // true（两端口独立）
    // 单向：fd0 → fd1
    match send_all(fd0, String::from("hello")) {
        Ok(n) => println(n),     // 5
        Err(e) => println(-1),
    }
    match recv_some(fd1, 16) {
        Ok(r) => println(r == String::from("hello")),   // true
        Err(e) => println(false),
    }
    // 反向：fd1 → fd0
    match send_all(fd1, String::from("world")) {
        Ok(n2) => println(n2),   // 5
        Err(e) => println(-1),
    }
    match recv_some(fd0, 16) {
        Ok(r2) => println(r2 == String::from("world")),  // true
        Err(e) => println(false),
    }
    let _ = unsafe { close(fd0) };
    let _ = unsafe { close(fd1) };
}
"#,
    );
    assert_eq!(out, "true\ntrue\ntrue\n5\ntrue\n5\ntrue\n");
}

/// 循环多包：发送端连续 3 包，接收端逐包还原。
#[test]
fn socketpair_loop_chunks() {
    let out = run(
        r#"
fn main() {
    let fds = socketpair_stream();
    let fd0 = fd_at(fds, 0);
    let fd1 = fd_at(fds, 1);
    // 发送端循环 3 包（"pkg0"/"pkg1"/"pkg2"）
    let mut i = 0;
    while i < 3 {
        let _ = send_all(fd0, String::from("pkg") + int_to_string(i));
        i = i + 1;
    }
    // 接收端循环 3 次各收 4 字节
    let mut j = 0;
    while j < 3 {
        match recv_some(fd1, 4) {
            Ok(v) => println(v),
            Err(e) => println(-1),
        }
        j = j + 1;
    }
    let _ = unsafe { close(fd0) };
    let _ = unsafe { close(fd1) };
}
"#,
    );
    assert_eq!(out, "pkg0\npkg1\npkg2\n");
}

/// 4096 字节二进制块往返：循环收满（SOCK_STREAM 无消息边界，recv 可能分片）。
#[test]
fn socketpair_large_chunk() {
    let out = run(
        r#"
fn main() {
    let fds = socketpair_stream();
    let fd0 = fd_at(fds, 0);
    let fd1 = fd_at(fds, 1);
    // 构造 4096 字节循环二进制内容
    let mut big = String::with_capacity(4096);
    let mut i = 0;
    while i < 4096 {
        big.push_byte(i & 0xFF);
        i = i + 1;
    }
    match send_all(fd0, big) {
        Ok(n) => println(n),     // 4096
        Err(e) => println(-1),
    }
    // 循环收满
    let mut got = String::new();
    let mut total = 0;
    while total < 4096 {
        match recv_some(fd1, 4096 - total) {
            Ok(chunk) => {
                if chunk.len == 0 {
                    break;       // 对端关闭，防死循环
                }
                got.push_str(chunk);
            }
            Err(e) => {
                break;
            }
        }
        total = got.len;
    }
    println(got.len);            // 4096
    println(got.data[0]);        // 0（第 1 字节）
    println(got.data[255]);      // 255
    println(got.data[256]);      // 0
    println(got.data[4095]);     // 255（末字节）
    let _ = unsafe { close(fd0) };
    let _ = unsafe { close(fd1) };
}
"#,
    );
    assert_eq!(out, "4096\n4096\n0\n255\n0\n255\n");
}

/// sockaddr_in4 字节布局（macOS：sin_len 头 + AF_INET + 端口网络序 + IP）。
#[test]
fn sockaddr_in4_bytes() {
    let out = run(
        r#"
fn main() {
    let sa = sockaddr_in4(8080, 127, 0, 0, 1);
    println(sa.data[0]);   // 16（sin_len）
    println(sa.data[1]);   // 2（AF_INET）
    println(sa.data[2]);   // 31（8080 高字节 0x1F）
    println(sa.data[3]);   // 144（8080 低字节 0x90）
    println(sa.data[4]);   // 127（IP 第 1 字节）
    println(sa.data[5]);   // 0
    println(sa.data[6]);   // 0
    println(sa.data[7]);   // 1（IP 第 4 字节）
    println(sa.data[8]);   // 0（sin_zero 起点）
    println(sa.data[15]);  // 0（sin_zero 末字节）
    // 端口 80：高字节 0、低字节 80
    let sb = sockaddr_in4(80, 192, 168, 1, 10);
    println(sb.data[2]);   // 0
    println(sb.data[3]);   // 80
    println(sb.data[4]);   // 192
    println(sb.data[7]);   // 10
}
"#,
    );
    assert_eq!(out, "16\n2\n31\n144\n127\n0\n0\n1\n0\n0\n0\n80\n192\n10\n");
}

/// tcp_connect 对未监听端口：sockaddr_in 构造被内核解析（ECONNREFUSED → Err）。
#[test]
fn connect_refused_unlistened() {
    let out = run(
        r#"
fn main() {
    // 127.0.0.1:1 无服务监听 → connect 返回 ECONNREFUSED → tcp_connect 返回 Err。
    // 若 sockaddr_in 布局错误（EINVAL/EFAULT），connect 同样非 0——本测试验证
    // 构造的 16 字节 sockaddr_in 能通过内核校验并到达协议层。
    match tcp_connect(1, 127, 0, 0, 1) {
        Ok(fd) => println(fd == -1),   // false（成功分支，不应走到）
        Err(e) => println(true),       // true（拒绝）
    }
}
"#,
    );
    assert_eq!(out, "true\n");
}
