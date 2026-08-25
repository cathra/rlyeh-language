// ===== net 模块（B3 完成 + O1–O3，2026-08）：socket 字节流 + 字节序工具 =====
// 目录化（2026-08）：原 net.rl 拆分 →
//   net/module.rl     （模块根：socket 自由函数 socketpair_stream/fd_at/send_all/recv_some/tcp_connect/hostname）
//   net/byteorder.rl（字节打包：htons/sockaddr_in4_with_layout/sockaddr_in4/int_buf4/octets_to_string/parse_sockaddr）
//   net/addr.rl    （Ipv4Octets/ipv4_octets/SocketAddr/Shutdown）
//   net/tcp.rl     （TcpStream/TcpListener）
//   net/http.rl    （ParsedUrl/parse_url/read_all/parse_status/Response/HttpClient/parse_response）
// 位运算全链路打通（& | ^ << >>）后启用：
// - sockaddr_in 经 String 缓冲逐字节打包（sockaddr_in4）
// - socketpair 的 fd 数组经 String 缓冲字节解释（fd_at，小端 int32）
// - socket/connect/close 返回 int，以 `-> i32` 声明（extern_ret32 sext 清洗）
// - send/recv 返回 ssize_t（i64 承载），buf 为 String（codegen 取 data 指针）
// 注意：sockaddr_in4 按 macOS 布局（offset 0 = sin_len）；Linux 无 sin_len
// （offset 0-1 = sin_family），移植时需调整（条件编译待 cfg 支持）。
// 底层 extern（socketpair/socket/connect/close/r#send/r#recv/
// __rlyeh_target_os/gethostname）声明于根模块 core.rl 的 extern 集中区。

// 声明顺序注意：签名类型在收集期解析（单遍），被引用的模块须先声明。
// byteorder::parse_sockaddr 返回 net::addr::SocketAddr → addr 先于 byteorder。
module addr;
module byteorder;
module tcp;
module http;

// 创建全双工字节流套接字对（AF_UNIX SOCK_STREAM，无需 sockaddr）。
// 返回承载 fd 数组的 8 字节缓冲（两个 int32）；经 fd_at 读取。
// WASI 下无 socket API：返回空缓冲（fd_at 读取到 0），明确禁用文档化（L4a）。
fn socketpair_stream() -> String {
    if __rlyeh_target_os() == 5 {
        return String::with_capacity(8);
    }
    let fds = String::with_capacity(8);
    let _ = socketpair(1, 1, 0, fds);
    fds
}

// 读取 fd 数组缓冲中第 k 个 int（k = 0 / 1），小端字节解释（x86-64 布局：
// int32 低字节在前，fd 为小正整数，符号位不触发）。
fn fd_at(fds: String, k: i64) -> i64 {
    let base = k * 4;
    fds.data[base] | (fds.data[base + 1] << 8) | (fds.data[base + 2] << 16) | (fds.data[base + 3] << 24)
}

// M3b（2026-08）：net 自由函数 Result 化——返回 `Result<T, IoError>`，
// 失败不再用 "0 / -1 / 空串" 哨兵值。
// 发送整段内容（len 字节）；成功返回实际发送字节数，失败返回 Err(IoError)。
// WASI 下无 socket API：返回 Err（禁用文档化，L4a）。
fn send_all(fd: i64, buf: String) -> Result<i64, io::error::IoError> {
    if __rlyeh_target_os() == 5 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::Other,
            String::from("socket API disabled on WASI"),
        ));
    }
    let n = r#send(fd, buf, buf.len, 0);
    if n < 0 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::Other,
            String::from("send failed"),
        ));
    }
    Result::Ok(n)
}

// 接收至多 cap 字节；成功返回按实际字节数设 len 的内容，失败返回 Err(IoError)。
// WASI 下无 socket API：返回 Err（禁用文档化，L4a）。
fn recv_some(fd: i64, cap: i64) -> Result<String, io::error::IoError> {
    if __rlyeh_target_os() == 5 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::Other,
            String::from("socket API disabled on WASI"),
        ));
    }
    let mut buf = String::with_capacity(cap);
    let n = r#recv(fd, buf, cap, 0);
    if n < 0 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::Other,
            String::from("recv failed"),
        ));
    }
    buf.len = n;
    Result::Ok(buf)
}

// M3b：TCP 连接（阻塞）：成功返回 fd（>0），失败返回 Err(IoError)（已 close 释放）。
// WASI 下无 socket API：返回 Err（禁用文档化，L4a）。
fn tcp_connect(port: i64, a: i64, b: i64, c: i64, d: i64) -> Result<i64, io::error::IoError> {
    if __rlyeh_target_os() == 5 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::Other,
            String::from("socket API disabled on WASI"),
        ));
    }
    let fd = socket(2, 1, 0);   // AF_INET=2, SOCK_STREAM=1
    if fd < 0 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::Other,
            String::from("socket creation failed"),
        ));
    }
    let sa = net::byteorder::sockaddr_in4(port, a, b, c, d);
    let r = connect(fd, sa, 16);
    if r != 0 {
        let _ = close(fd);
        return Result::Err(IoError::new(
            io::error::IoErrorKind::Other,
            String::from("connect failed"),
        ));
    }
    Result::Ok(fd)
}

// M3b：本机主机名。
// gethostname 返回 int（i32），以 i64 声明时高位未定义，故不依赖返回值，
// 仅扫描缓冲内首个 NUL 定位实际长度（成功时 gethostname 保证 NUL 结尾）。
// WASI 下无 gethostname API：返回 Err（禁用文档化，L4a）。
fn hostname() -> Result<String, io::error::IoError> {
    if __rlyeh_target_os() == 5 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::Other,
            String::from("gethostname disabled on WASI"),
        ));
    }
    let mut buf = String::with_capacity(256);
    let _r = gethostname(buf, 256);
    // 定位 NUL 终止位置作为实际长度（找不到则回退 255）
    let mut i = 0;
    while i < 256 {
        if buf.data[i] == 0 {
            break;
        }
        i = i + 1;
    }
    buf.len = i;
    Result::Ok(buf)
}
