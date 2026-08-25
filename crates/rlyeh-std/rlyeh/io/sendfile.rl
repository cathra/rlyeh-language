// io/sendfile.rl：内核零拷贝文件传输（std-lib.md §4.5）。
// R3（2026-08）：
// - 语言侧统一 API `sendfile(out_fd, in_fd, offset, count)`；
//   sendfile(2) 的平台签名差异（Linux 4 参 / macOS 6 参）由 driver 注入的
//   `__rlyeh_sendfile` 屏蔽（见 rlyeh-driver lib.rs platform_builtin_ir）。
// - offset 为文件内起始偏移（字节），count == 0 表示发送到 EOF；
//   返回实际发送字节数，失败返回 Err(IoError)。
// - offset 以 8 字节小端 int64 缓冲承载（off_t 指针语义，两个平台通用）。
// 注意：WASI（码 5）无 sendfile(2)，返回 Err（禁用文档化，L4a）。

// 发送 in_fd 从 offset 起的内容到 out_fd（out_fd 须为 socket）。
// count == 0 表示发送到文件末尾；返回实际发送字节数；失败返回 Err(IoError)。
fn sendfile(out_fd: i64, in_fd: i64, offset: i64, count: i64) -> Result<i64, io::error::IoError> {
    if __rlyeh_target_os() == 5 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::Other,
            String::from("sendfile disabled on WASI"),
        ));
    }
    let mut off = String::with_capacity(8);
    off.push_byte(offset & 0xFF);
    off.push_byte((offset >> 8) & 0xFF);
    off.push_byte((offset >> 16) & 0xFF);
    off.push_byte((offset >> 24) & 0xFF);
    off.push_byte((offset >> 32) & 0xFF);
    off.push_byte((offset >> 40) & 0xFF);
    off.push_byte((offset >> 48) & 0xFF);
    off.push_byte((offset >> 56) & 0xFF);
    let r = __rlyeh_sendfile(out_fd, in_fd, off, count);
    if r < 0 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::Other,
            String::from("sendfile failed"),
        ));
    }
    Result::Ok(r)
}
