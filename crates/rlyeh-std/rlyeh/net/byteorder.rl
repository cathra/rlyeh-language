// net/byteorder.rl：sockaddr_in 字节打包工具（主机字节序 ↔ 网络字节序）。
// 目录化（2026-08）：由原 net.rl 拆分。符号完整路径 net::byteorder::sockaddr_in4 等。

// 主机字节序 → 网络字节序（大端）。纯位运算实现（不依赖 htons extern）。
fn htons(v: i64) -> i64 {
    ((v & 0xFF) << 8) | ((v >> 8) & 0xFF)
}

// 构造 IPv4 sockaddr_in（16 字节）。a.b.c.d 点分四字节，port 为主机字节序。
// has_sin_len 选择平台布局（由 __rlyeh_target_os 决定）：
//   macOS（true）： 0 = sin_len(16)，1 = AF_INET(2)
//   Linux（false）：0-1 = sin_family（uint16 小端 0x0002 → 2, 0），无 sin_len
// 之后 2-3 = 端口（大端/网络序），4-7 = IP（大端/网络序），8-15 = sin_zero。
// 注意：sin_port/sin_addr 直接按大端布局拆字节（高字节在前），
// 不经过 htons 数值转换——htons 返回字节交换后的数值，若再按高字节先存
// 会把字节序反向（8080 会错成 0x901F 布局）。
fn sockaddr_in4_with_layout(has_sin_len: bool, port: i64, a: i64, b: i64, c: i64, d: i64) -> String {
    let mut sa = String::with_capacity(16);
    if has_sin_len {
        sa.push_byte(16);               // sin_len（macOS 特有）
        sa.push_byte(2);                // AF_INET
    } else {
        sa.push_byte(2);                // sin_family 低字节（小端 0x0002）
        sa.push_byte(0);                // sin_family 高字节
    }
    sa.push_byte((port >> 8) & 0xFF);  // 端口高字节（大端）
    sa.push_byte(port & 0xFF);         // 端口低字节
    sa.push_byte(a); sa.push_byte(b); sa.push_byte(c); sa.push_byte(d);
    sa.push_byte(0); sa.push_byte(0); sa.push_byte(0); sa.push_byte(0);
    sa.push_byte(0); sa.push_byte(0); sa.push_byte(0); sa.push_byte(0);
    sa
}

// 平台自适应 sockaddr_in4（按目标 OS 选布局）。
fn sockaddr_in4(port: i64, a: i64, b: i64, c: i64, d: i64) -> String {
    net::byteorder::sockaddr_in4_with_layout(__rlyeh_target_os() == 2, port, a, b, c, d)
}

// 4 字节 → 点分字符串（getsockname / accept 回填解析的反向）。
fn octets_to_string(a: i64, b: i64, c: i64, d: i64) -> String {
    int_to_string(a) + String::from(".") + int_to_string(b) + String::from(".") + int_to_string(c) + String::from(".") + int_to_string(d)
}

// int32 小端缓冲（setsockopt 的 optval / accept/getsockname 的 socklen_t 初值）。
fn int_buf4(v: i64) -> String {
    let mut b = String::with_capacity(4);
    b.push_byte(v & 0xFF);
    b.push_byte((v >> 8) & 0xFF);
    b.push_byte((v >> 16) & 0xFF);
    b.push_byte((v >> 24) & 0xFF);
    b
}

// 从 sockaddr_in 缓冲解析 net::addr::SocketAddr（字节 2-3 端口大端，4-7 IP；0/1 为 family）。
fn parse_sockaddr(buf: String) -> net::addr::SocketAddr {
    let port = (buf.data[2] << 8) | buf.data[3];
    let ip = net::byteorder::octets_to_string(buf.data[4], buf.data[5], buf.data[6], buf.data[7]);
    net::addr::SocketAddr { ip: ip, port: port }
}
