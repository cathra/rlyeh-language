// net/udp.rl：UDP 数据报（std-lib.md §5.6）。
// Y7（2026-08）：UdpSocket 目标 API——bind/send_to/recv_from/local_addr。
// - sendto/recvfrom 为无状态原语（声明于 core.rl extern 集中区）；
// - sockaddr_in 构造/解析复用 net::byteorder（O1a 平台双布局）；
// - 数据报返回经 UdpPacket 结构体承载（data + 源地址 from）；
// - WASI 下无 socket API：方法返回 Err（禁用文档化，L4a 与 TCP 一致）。

// Y7：UDP 数据报承载（recv_from 返回值）。
struct UdpPacket {
    data: String,                          // 报文内容（按实际字节数设 len）
    from: net::addr::SocketAddr,           // 源地址（recvfrom 回填解析）
}

// Y7：UDP 套接字（IPv4，面向无连接）。fd + 绑定地址。
struct UdpSocket {
    fd: i64,
    addr: net::addr::SocketAddr,           // 绑定地址（bind(端口 0) 后经 getsockname 读实际端口）
}

impl UdpSocket {
    // 绑定到 addr（IPv4）。port 0 = 内核分配端口，随后用 local_addr() 读实际值。
    fn bind(addr: net::addr::SocketAddr) -> Result<net::udp::UdpSocket, io::error::IoError> {
        if __rlyeh_target_os() == 5 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("socket API disabled on WASI"),
            ));
        }
        let fd = socket(2, 2, 0);   // AF_INET=2, SOCK_DGRAM=2
        if fd < 0 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("socket creation failed"),
            ));
        }
        let sa = addr.to_sockaddr();
        let r = bind(fd, sa, 16);
        if r != 0 {
            let _ = close(fd);
            return Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("bind failed"),
            ));
        }
        // 端口 0：getsockname 读实际绑定端口
        let mut bound = addr;
        if addr.port == 0 {
            let mut buf = String::with_capacity(16);
            let mut len_buf = net::byteorder::int_buf4(16);
            let gr = getsockname(fd, buf, len_buf);
            if gr != 0 {
                let _ = close(fd);
                return Result::Err(IoError::new(
                    io::error::IoErrorKind::Other,
                    String::from("getsockname failed"),
                ));
            }
            bound = net::byteorder::parse_sockaddr(buf);
        }
        Result::Ok(net::udp::UdpSocket { fd: fd, addr: bound })
    }

    // 绑定地址（bind(端口 0) 时为内核分配的实际端口）。
    fn local_addr(&self) -> net::addr::SocketAddr {
        self.addr
    }

    // 发送一个数据报到目标地址（sendto，无连接）。返回实际发送字节数。
    fn send_to(&self, buf: String, addr: net::addr::SocketAddr) -> Result<i64, io::error::IoError> {
        if __rlyeh_target_os() == 5 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("socket API disabled on WASI"),
            ));
        }
        let sa = addr.to_sockaddr();
        let n = sendto(self.fd, buf, buf.len, 0, sa, 16);
        if n < 0 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("sendto failed"),
            ));
        }
        Result::Ok(n)
    }

    // 接收一个数据报（至多 cap 字节，丢弃更大报文）。返回 UdpPacket{data, from}。
    fn recv_from(&self, cap: i64) -> Result<net::udp::UdpPacket, io::error::IoError> {
        if __rlyeh_target_os() == 5 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("socket API disabled on WASI"),
            ));
        }
        let mut buf = String::with_capacity(cap);
        let mut addr_buf = String::with_capacity(16);
        let mut len_buf = net::byteorder::int_buf4(16);   // socklen_t 初值 = 16
        let n = recvfrom(self.fd, buf, cap, 0, addr_buf, len_buf);
        if n < 0 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("recvfrom failed"),
            ));
        }
        buf.len = n;
        let peer = net::byteorder::parse_sockaddr(addr_buf);
        Result::Ok(net::udp::UdpPacket { data: buf, from: peer })
    }

    // R2（2026-08）：非阻塞模式设置/查询（fcntl O_NONBLOCK，io::nio 转发）。
    fn set_nonblocking(&self, nonblocking: bool) -> Result<i64, io::error::IoError> {
        io::nio::set_nonblocking(self.fd, nonblocking)
    }
    fn is_nonblocking(&self) -> Result<bool, io::error::IoError> {
        io::nio::is_nonblocking(self.fd)
    }
}
