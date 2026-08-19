//! zeta-std NIO 与 sendfile 集成测试。

#![cfg(unix)]

use std::fs::File;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;
use std::time::Duration;

use zeta_std::nio::{is_nonblocking, sendfile, set_nonblocking, Interest, Poller};

/// sendfile 原生支持平台（Linux 与 macOS/BSD）。
#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
fn temp_file(contents: &[u8]) -> (std::path::PathBuf, File) {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let path = std::env::temp_dir().join(format!(
        "zeta-nio-test-{}-{}.tmp",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, contents).unwrap();
    let file = File::open(&path).unwrap();
    (path, file)
}

fn pair() -> (UnixStream, UnixStream) {
    UnixStream::pair().expect("socketpair")
}

// ---------------------------------------------------------------------------
// 非阻塞模式
// ---------------------------------------------------------------------------

#[test]
fn test_set_nonblocking() {
    let (mut a, _b) = pair();
    let fd = a.as_raw_fd();

    set_nonblocking(fd, true).unwrap();
    assert!(is_nonblocking(fd).unwrap());

    // 空 socket 非阻塞读应立即返回 WouldBlock
    let mut buf = [0u8; 16];
    let err = a.read(&mut buf).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::WouldBlock);

    set_nonblocking(fd, false).unwrap();
    assert!(!is_nonblocking(fd).unwrap());
}

// ---------------------------------------------------------------------------
// sendfile 零拷贝传输
// ---------------------------------------------------------------------------

#[test]
#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
fn test_sendfile_all() {
    let payload = b"hello sendfile world";
    let (path, file) = temp_file(payload);
    let (sock_a, mut sock_b) = pair();

    // count == 0 表示发送到 EOF
    let sent = sendfile(sock_a.as_raw_fd(), file.as_raw_fd(), 0, 0).unwrap();
    assert_eq!(sent, payload.len());

    let mut buf = vec![0u8; payload.len()];
    sock_b.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, payload);

    std::fs::remove_file(path).ok();
}

#[test]
#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
fn test_sendfile_offset() {
    let payload = b"0123456789abcdef";
    let (path, file) = temp_file(payload);
    let (sock_a, mut sock_b) = pair();

    // 从 offset 6 发送到 EOF
    let sent = sendfile(sock_a.as_raw_fd(), file.as_raw_fd(), 6, 0).unwrap();
    assert_eq!(sent, payload.len() - 6);

    let mut buf = vec![0u8; sent];
    sock_b.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"6789abcdef");

    std::fs::remove_file(path).ok();
}

#[test]
#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
fn test_sendfile_count() {
    let payload = b"hello sendfile world";
    let (path, file) = temp_file(payload);
    let (sock_a, mut sock_b) = pair();

    // 只发送前 5 字节
    let sent = sendfile(sock_a.as_raw_fd(), file.as_raw_fd(), 0, 5).unwrap();
    assert_eq!(sent, 5);

    let mut buf = vec![0u8; 5];
    sock_b.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"hello");

    std::fs::remove_file(path).ok();
}

// ---------------------------------------------------------------------------
// Poller 事件轮询
// ---------------------------------------------------------------------------

#[test]
fn test_poller_readable_timeout() {
    let (a, mut b) = pair();
    let poller = Poller::new().unwrap();
    let fd = a.as_raw_fd();

    poller.register(fd, 42, Interest::Readable).unwrap();

    // 无数据：短超时应返回 0
    let mut events = Vec::new();
    let n = poller
        .poll(&mut events, Some(Duration::from_millis(10)))
        .unwrap();
    assert_eq!(n, 0);

    // 写入后应触发可读事件
    b.write_all(b"x").unwrap();
    let n = poller
        .poll(&mut events, Some(Duration::from_millis(200)))
        .unwrap();
    assert_eq!(n, 1);
    assert_eq!(events[0].token, 42);
    assert!(events[0].is_readable());
}

#[test]
fn test_poller_reregister_writable() {
    let (a, _b) = pair();
    let poller = Poller::new().unwrap();
    let fd = a.as_raw_fd();

    poller.register(fd, 1, Interest::Readable).unwrap();
    poller.reregister(fd, 7, Interest::Writable).unwrap();

    // 空 socket 立即可写
    let mut events = Vec::new();
    let n = poller
        .poll(&mut events, Some(Duration::from_millis(200)))
        .unwrap();
    assert_eq!(n, 1);
    assert_eq!(events[0].token, 7);
    assert!(events[0].is_writable());
}

#[test]
fn test_poller_deregister() {
    let (a, mut b) = pair();
    let poller = Poller::new().unwrap();
    let fd = a.as_raw_fd();

    poller.register(fd, 5, Interest::Readable).unwrap();
    poller.deregister(fd).unwrap();

    // 注销后即使有数据也不应触发
    b.write_all(b"x").unwrap();
    let mut events = Vec::new();
    let n = poller
        .poll(&mut events, Some(Duration::from_millis(50)))
        .unwrap();
    assert_eq!(n, 0);
}
