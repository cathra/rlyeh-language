// net/addr.zeta：IP 地址与端口承载（std-lib.md §5.1）。
// 目录化（2026-08）：由原 net.zeta 拆分。符号完整路径 net::addr::SocketAddr 等。

// O1a：IPv4 八位组承载（ipv4_octets 解析结果）。
struct Ipv4Octets {
    a: i64,
    b: i64,
    c: i64,
    d: i64,
}

// 解析点分 IPv4 字符串 "a.b.c.d" → 四段整数；格式非法返回 Err(InvalidInput)。
fn ipv4_octets(ip: String) -> Result<net::addr::Ipv4Octets, io::error::IoError> {
    let mut a = 0;
    let mut b = 0;
    let mut c = 0;
    let mut d = 0;
    let mut seg = 0;    // 当前段索引 0..3（'c' 段 2 后遇 '.' 报错）
    let mut val = 0;    // 当前段累计值
    let mut seen = 0;   // 当前段数字个数（防御空段 "1..2.3"）
    let mut i = 0;
    while i < ip.len {
        let ch = ip.data[i];
        if ch >= 48 && ch <= 57 {
            val = val * 10 + (ch - 48);
            seen = seen + 1;
        } else if ch == 46 {
            if seen == 0 || seg == 3 {
                return Result::Err(IoError::new(
                    io::error::IoErrorKind::InvalidInput,
                    String::from("invalid IPv4 address"),
                ));
            }
            if seg == 0 {
                a = val;
            } else if seg == 1 {
                b = val;
            } else {
                c = val;
            }
            seg = seg + 1;
            val = 0;
            seen = 0;
        } else {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::InvalidInput,
                String::from("invalid IPv4 address"),
            ));
        }
        i = i + 1;
    }
    if seen == 0 || seg != 3 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::InvalidInput,
            String::from("invalid IPv4 address"),
        ));
    }
    d = val;
    Result::Ok(net::addr::Ipv4Octets { a: a, b: b, c: c, d: d })
}

// O1a：IP + 端口。`parse` 支持 "ip:port" 字符串；`to_sockaddr` 构造 sockaddr_in。
struct SocketAddr {
    ip: String,
    port: i64,
}

impl SocketAddr {
    fn new(ip: String, port: i64) -> net::addr::SocketAddr {
        net::addr::SocketAddr { ip: ip, port: port }
    }
    fn ip(&self) -> String {
        self.ip
    }
    fn port(&self) -> i64 {
        self.port
    }
    // 解析 "ip:port"（IPv4，最后一个 ':' 分隔端口）；格式非法返回 Err。
    fn parse(s: String) -> Result<net::addr::SocketAddr, io::error::IoError> {
        let mut i = s.len - 1;
        let mut colon = -1;
        while i >= 0 {
            if s.data[i] == 58 {
                colon = i;
                break;
            }
            i = i - 1;
        }
        if colon <= 0 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::InvalidInput,
                String::from("invalid socket addr (missing colon)"),
            ));
        }
        let ip = s[0..<colon];
        let port = string_to_int(s[(colon + 1)..<s.len]);
        let _ = net::addr::ipv4_octets(ip)?;
        // 模块限定关联函数调用不可用，改手工字面量构造（与 HttpClient 一致）。
        Result::Ok(net::addr::SocketAddr { ip: ip, port: port })
    }
    // 构造 sockaddr_in（IPv4 解析失败回退 0.0.0.0，bind/connect 将失败）。
    fn to_sockaddr(&self) -> String {
        let o = match net::addr::ipv4_octets(self.ip) {
            Result::Ok(x) => x,
            Result::Err(_) => return net::byteorder::sockaddr_in4(0, 0, 0, 0, 0),
        };
        net::byteorder::sockaddr_in4(self.port, o.a, o.b, o.c, o.d)
    }
}

// O2：关闭方向（C shutdown 的 how 参数）。
enum Shutdown {
    Read,
    Write,
    Both,
}
