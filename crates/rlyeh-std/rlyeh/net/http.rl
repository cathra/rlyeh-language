// net/http.rl：HTTP 同步 MVP（std-lib.md §5.2）。
// 目录化（2026-08）：由原 net.rl 拆分。符号完整路径 net::http::HttpClient 等。
// MVP 简化：URL 仅支持 `http://host[:port]/path`（IPv4）；每次请求新连接
// （无连接复用）；响应解析不依赖 Content-Length（依赖 Connection: close
// 的 EOF 终止 body）；无重定向/分块传输。json 反序列化用 L2 自由函数
// `json::parse::<T>(resp.text())`（方法 turbofish 未支持）。

// URL 解析结果承载。
struct ParsedUrl {
    host: String,
    port: i64,
    path: String,
}

// 解析 `http://host[:port]/path` → 主机/端口/路径；默认端口 80、路径 "/"。
fn parse_url(url: String) -> Result<net::http::ParsedUrl, io::error::IoError> {
    let mut rest = url;
    if rest.len >= 7 && rest[0..<7] == "http://" {
        rest = rest[7..<rest.len];
    }
    let mut i = 0;
    let mut slash = -1;
    while i < rest.len {
        if rest.data[i] == 47 {
            slash = i;
            break;
        }
        i = i + 1;
    }
    let mut authority = rest;
    let mut path = String::from("/");
    if slash >= 0 {
        authority = rest[0..<slash];
        path = rest[slash..<rest.len];
    }
    let mut j = 0;
    let mut colon = -1;
    while j < authority.len {
        if authority.data[j] == 58 {
            colon = j;
            break;
        }
        j = j + 1;
    }
    let mut host = authority;
    let mut port = 80;
    if colon >= 0 {
        host = authority[0..<colon];
        port = string_to_int(authority[(colon + 1)..<authority.len]);
    }
    Result::Ok(net::http::ParsedUrl { host: host, port: port, path: path })
}

// 读取连接直至 EOF（Connection: close 语义）。
fn read_all(stream: net::tcp::TcpStream) -> Result<String, io::error::IoError> {
    let mut buf = String::new();
    loop {
        let tmp = stream.read(1024)?;
        if tmp.len == 0 {
            break;
        }
        buf = buf + tmp;
    }
    Result::Ok(buf)
}

// 从响应头部首行提取状态码（"HTTP/1.1 200 OK" → 200）。
fn parse_status(head: String) -> i64 {
    let mut i = 0;
    let mut sp = -1;
    while i < head.len {
        if head.data[i] == 32 {
            sp = i;
            break;
        }
        i = i + 1;
    }
    if sp < 0 {
        return 0;
    }
    let mut j = sp + 1;
    while j < head.len && head.data[j] == 32 {
        j = j + 1;
    }
    string_to_int(head[j..<head.len])
}

// O3b：HTTP 响应。json 反序列化：`json::parse::<T>(resp.text())`。
struct Response {
    status: i64,
    body: String,
}

// O3a：HTTP 客户端。MVP 无状态（`_unit` 占位字段：空结构体构造 `{}` 与块歧义，parser 不支持）。
struct HttpClient {
    _unit: i64,
}

impl HttpClient {
    fn new() -> net::http::HttpClient {
        net::http::HttpClient { _unit: 0 }
    }
    fn get(url: String) -> Result<net::http::Response, io::error::IoError> {
        if __rlyeh_target_os() == 5 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("HTTP disabled on WASI"),
            ));
        }
        let u = net::http::parse_url(url)?;
        // 复用 tcp_connect 自由函数（模块限定关联函数调用 `net::tcp::TcpStream::connect` 不可用）
        let oct = match ipv4_octets(u.host) {
            Result::Ok(x) => x,
            Result::Err(e) => return Result::Err(e),
        };
        let fd = match tcp_connect(u.port, oct.a, oct.b, oct.c, oct.d) {
            Result::Ok(f) => f,
            Result::Err(e) => return Result::Err(e),
        };
        let stream = net::tcp::TcpStream { fd: fd, addr: net::addr::SocketAddr { ip: u.host, port: u.port } };
        let req = String::from("GET ") + u.path + String::from(" HTTP/1.1\r\nHost: ") + u.host + String::from("\r\nConnection: close\r\n\r\n");
        let _ = stream.write(req)?;
        let all = net::http::read_all(stream)?;
        net::http::parse_response(all)
    }
    fn post(url: String, body: String) -> Result<net::http::Response, io::error::IoError> {
        if __rlyeh_target_os() == 5 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("HTTP disabled on WASI"),
            ));
        }
        let u = net::http::parse_url(url)?;
        let oct = match ipv4_octets(u.host) {
            Result::Ok(x) => x,
            Result::Err(e) => return Result::Err(e),
        };
        let fd = match tcp_connect(u.port, oct.a, oct.b, oct.c, oct.d) {
            Result::Ok(f) => f,
            Result::Err(e) => return Result::Err(e),
        };
        let stream = net::tcp::TcpStream { fd: fd, addr: net::addr::SocketAddr { ip: u.host, port: u.port } };
        let req = String::from("POST ") + u.path + String::from(" HTTP/1.1\r\nHost: ") + u.host + String::from("\r\nContent-Length: ") + int_to_string(body.len) + String::from("\r\nConnection: close\r\n\r\n") + body;
        let _ = stream.write(req)?;
        let all = net::http::read_all(stream)?;
        net::http::parse_response(all)
    }
    // S3b：异步 GET / POST（MVP 退化——同步语义，等价 get/post；事件驱动版规划随
    // S3 事件循环 + §4.4 NIO：io_uring/epoll 注册 + Future 挂起，R1 Poller 先行）。
    fn get_async(url: String) -> Result<net::http::Response, io::error::IoError> {
        HttpClient::get(url)
    }
    fn post_async(url: String, body: String) -> Result<net::http::Response, io::error::IoError> {
        HttpClient::post(url, body)
    }
}

impl Response {
    fn status(&self) -> i64 {
        self.status
    }
    fn text(&self) -> String {
        self.body
    }
}

// 解析完整响应：`\r\n\r\n` 分隔头部/body，body 保留原样（含换行）。
fn parse_response(resp: String) -> Result<net::http::Response, io::error::IoError> {
    let mut i = 0;
    let mut sep = -1;
    while i + 3 < resp.len {
        if resp.data[i] == 13 && resp.data[i + 1] == 10 && resp.data[i + 2] == 13 && resp.data[i + 3] == 10 {
            sep = i;
            break;
        }
        i = i + 1;
    }
    if sep < 0 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::Other,
            String::from("malformed HTTP response (no header terminator)"),
        ));
    }
    let head = resp[0..<sep];
    let body = resp[(sep + 4)..<resp.len];
    let status = net::http::parse_status(head);
    Result::Ok(net::http::Response { status: status, body: body })
}
