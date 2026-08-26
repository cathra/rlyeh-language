// net/http.rl：HTTP 同步客户端（std-lib.md §5.2）。
// 目录化（2026-08）：由原 net.rl 拆分。符号完整路径 net::http::HttpClient 等。
// MVP 简化：URL 仅支持 `http://host[:port]/path`（IPv4）。
// Y3（2026-08）：连接复用——HttpClient 实例持有 keep-alive 空闲连接
// （同 host:port 连续请求复用，替代每请求新建 + close），请求头
// `Connection: keep-alive`；响应 body 优先按 Content-Length 精确读取
// （keep-alive 必需），无 Content-Length 回退 EOF 终止（兼容 Connection: close
// 服务器）；复用连接失效（发送/读取错误，如服务器 keep-alive 超时关闭）
// 自动丢弃并新建连接重试一次。无重定向/分块传输。
// json 反序列化用 L2 自由函数 `json::parse::<T>(resp.text())`（方法 turbofish 未支持）。

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

// O3b：HTTP 响应。json 反序列化：`json::parse::<T>(resp.text())`。
struct Response {
    status: i64,
    body: String,
}

// 找 `\r\n\r\n`（头部终止符）的位置；未找到返回 -1。
fn find_header_end(buf: String) -> i64 {
    let mut i = 0;
    while i + 3 < buf.len {
        if buf.data[i] == 13 && buf.data[i + 1] == 10 && buf.data[i + 2] == 13 && buf.data[i + 3] == 10 {
            return i;
        }
        i = i + 1;
    }
    -1
}

// 从响应头部解析 Content-Length（大小写不敏感）；无该头返回 -1。
fn parse_content_length(head: String) -> i64 {
    let prefix = String::from("content-length:");
    let mut i = 0;
    while i < head.len {
        if i > 0 && head.data[i - 1] != 10 {
            i = i + 1;
            continue;
        }
        // 行首：比较前缀 "content-length:"（ASCII 大小写不敏感）
        let mut j = 0;
        let mut hit = true;
        while j < prefix.len && i + j < head.len {
            let c = head.data[i + j];
            let lc = if c >= 65 && c <= 90 { c + 32 } else { c };
            if lc != prefix.data[j] {
                hit = false;
                break;
            }
            j = j + 1;
        }
        if hit {
            let mut k = i + prefix.len;
            while k < head.len && head.data[k] == 32 {
                k = k + 1;
            }
            let mut e = k;
            while e < head.len && head.data[e] != 13 {
                e = e + 1;
            }
            return string_to_int(head[k..<e]);
        }
        i = i + 1;
    }
    -1
}

// 读取完整响应（fd 不关闭）：头部到 `\r\n\r\n`；body 按 Content-Length
// 精确读取（keep-alive 必需，连接保持供复用）；无 Content-Length 时回退
// EOF 终止（兼容 Connection: close 服务器）。多读的字节丢弃（不缓冲——MVP
// 简化，无 pipelining）。
fn read_response(fd: i64) -> Result<net::http::Response, io::error::IoError> {
    let stream = net::tcp::TcpStream { fd: fd, addr: net::addr::SocketAddr { ip: String::new(), port: 0 } };
    let mut buf = String::new();
    let mut sep = -1;
    while sep < 0 {
        let tmp = match stream.read(1024) {
            Result::Ok(t) => t,
            Result::Err(e) => return Result::Err(e),
        };
        if tmp.len == 0 {
            break;
        }
        buf = buf + tmp;
        sep = net::http::find_header_end(buf);
    }
    if sep < 0 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::Other,
            String::from("malformed HTTP response (no header terminator)"),
        ));
    }
    let head = buf[0..<sep];
    let mut body = buf[(sep + 4)..<buf.len];
    let cl = net::http::parse_content_length(head);
    if cl >= 0 {
        while body.len < cl {
            let tmp = match stream.read(1024) {
                Result::Ok(t) => t,
                Result::Err(e) => return Result::Err(e),
            };
            if tmp.len == 0 {
                break;
            }
            body = body + tmp;
        }
        body = body[0..<cl];
    } else {
        loop {
            let tmp = match stream.read(1024) {
                Result::Ok(t) => t,
                Result::Err(e) => return Result::Err(e),
            };
            if tmp.len == 0 {
                break;
            }
            body = body + tmp;
        }
    }
    let status = net::http::parse_status(head);
    Result::Ok(net::http::Response { status: status, body: body })
}

// 单次请求（不管理连接池）：发请求（body 为空 = GET，非空 = POST，
// 均带 `Connection: keep-alive`）+ 读响应。连接复用失败检测由调用方处理。
fn request_one(fd: i64, host: String, path: String, body: String) -> Result<net::http::Response, io::error::IoError> {
    let mut req = String::new();
    if body.len == 0 {
        req = String::from("GET ") + path + String::from(" HTTP/1.1\r\nHost: ") + host + String::from("\r\nConnection: keep-alive\r\n\r\n");
    } else {
        req = String::from("POST ") + path + String::from(" HTTP/1.1\r\nHost: ") + host + String::from("\r\nContent-Length: ") + int_to_string(body.len) + String::from("\r\nConnection: keep-alive\r\n\r\n") + body;
    }
    let stream = net::tcp::TcpStream { fd: fd, addr: net::addr::SocketAddr { ip: host, port: 0 } };
    match stream.write(req) {
        Result::Err(e) => return Result::Err(e),
        Result::Ok(_) => net::http::read_response(fd),
    }
}

// W5：从完整响应文本提取 body——去掉头部（至 `\r\n\r\n`），按 Content-Length
// 截断（keep-alive 必需）；无 Content-Length 返回头部后的全部。
fn extract_body(buf: String) -> String {
    let sep = net::http::find_header_end(buf);
    if sep < 0 {
        return String::new();
    }
    let head = buf[0..<sep];
    let mut body = buf[(sep + 4)..<buf.len];
    let cl = net::http::parse_content_length(head);
    if cl >= 0 {
        if body.len > cl {
            body = body[0..<cl];
        }
    }
    body
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

// O3a + Y3：HTTP 客户端。Y3 起为有状态连接池——实例持有 keep-alive 空闲连接
// （conn_fd + host:port 标识），`get`/`post` 为实例方法（&mut self），同
// host:port 连续请求复用连接；`_unit` 占位字段保留（空结构体构造 `{}` 与块歧义）。
struct HttpClient {
    _unit: i64,
    conn_fd: i64,       // keep-alive 空闲连接 fd（-1 = 无）
    conn_host: String,  // 空闲连接 host
    conn_port: i64,     // 空闲连接 port
}

impl HttpClient {
    fn new() -> net::http::HttpClient {
        net::http::HttpClient { _unit: 0, conn_fd: -1, conn_host: String::new(), conn_port: 0 }
    }
    fn get(&mut self, url: String) -> Result<net::http::Response, io::error::IoError> {
        if __rlyeh_target_os() == 5 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("HTTP disabled on WASI"),
            ));
        }
        let u = net::http::parse_url(url)?;
        let reuse = self.conn_fd >= 0 && self.conn_host == u.host && self.conn_port == u.port;
        // 第一次尝试（可能复用空闲连接）
        let mut fd = -1;
        if reuse {
            fd = self.conn_fd;
        } else {
            // 复用 tcp_connect 自由函数（模块限定关联函数调用 `net::tcp::TcpStream::connect` 不可用）
            let oct = match ipv4_octets(u.host) {
                Result::Ok(x) => x,
                Result::Err(e) => return Result::Err(e),
            };
            match tcp_connect(u.port, oct.a, oct.b, oct.c, oct.d) {
                Result::Ok(f) => fd = f,
                Result::Err(e) => return Result::Err(e),
            };
        }
        match net::http::request_one(fd, u.host, u.path, String::new()) {
            Result::Ok(r) => {
                // 响应成功：连接保留供复用
                self.conn_fd = fd;
                self.conn_host = u.host;
                self.conn_port = u.port;
                return Result::Ok(r);
            }
            Result::Err(e) => {
                if !reuse {
                    return Result::Err(e);
                }
            }
        }
        // 复用连接失效（服务器 keep-alive 超时/主动关闭）：丢弃并新建重试一次
        self.conn_fd = -1;
        let oct = match ipv4_octets(u.host) {
            Result::Ok(x) => x,
            Result::Err(e) => return Result::Err(e),
        };
        let fd2 = match tcp_connect(u.port, oct.a, oct.b, oct.c, oct.d) {
            Result::Ok(f) => f,
            Result::Err(e) => return Result::Err(e),
        };
        // 尾部表达式（match 作表达式返回 Result）
        match net::http::request_one(fd2, u.host, u.path, String::new()) {
            Result::Ok(r) => {
                self.conn_fd = fd2;
                self.conn_host = u.host;
                self.conn_port = u.port;
                Result::Ok(r)
            }
            Result::Err(e) => return Result::Err(e),
        }
    }
    fn post(&mut self, url: String, body: String) -> Result<net::http::Response, io::error::IoError> {
        if __rlyeh_target_os() == 5 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("HTTP disabled on WASI"),
            ));
        }
        let u = net::http::parse_url(url)?;
        let reuse = self.conn_fd >= 0 && self.conn_host == u.host && self.conn_port == u.port;
        let mut fd = -1;
        if reuse {
            fd = self.conn_fd;
        } else {
            let oct = match ipv4_octets(u.host) {
                Result::Ok(x) => x,
                Result::Err(e) => return Result::Err(e),
            };
            match tcp_connect(u.port, oct.a, oct.b, oct.c, oct.d) {
                Result::Ok(f) => fd = f,
                Result::Err(e) => return Result::Err(e),
            };
        }
        match net::http::request_one(fd, u.host, u.path, body) {
            Result::Ok(r) => {
                self.conn_fd = fd;
                self.conn_host = u.host;
                self.conn_port = u.port;
                return Result::Ok(r);
            }
            Result::Err(e) => {
                if !reuse {
                    return Result::Err(e);
                }
            }
        }
        self.conn_fd = -1;
        let oct = match ipv4_octets(u.host) {
            Result::Ok(x) => x,
            Result::Err(e) => return Result::Err(e),
        };
        let fd2 = match tcp_connect(u.port, oct.a, oct.b, oct.c, oct.d) {
            Result::Ok(f) => f,
            Result::Err(e) => return Result::Err(e),
        };
        // 尾部表达式（match 作表达式返回 Result）
        match net::http::request_one(fd2, u.host, u.path, body) {
            Result::Ok(r) => {
                self.conn_fd = fd2;
                self.conn_host = u.host;
                self.conn_port = u.port;
                Result::Ok(r)
            }
            Result::Err(e) => return Result::Err(e),
        }
    }
    // W5：get_async/post_async 真异步 future 定义于 HttpClient 补充 impl
    // （见下方 `impl HttpClient`）。原同步退化移除。
}

impl Response {
    fn status(&self) -> i64 {
        self.status
    }
    fn text(&self) -> String {
        self.body
    }
}

// W5（2026-08-25）：异步 GET future（`HttpClient::get_async` 返回值）。
// 简化真异步：首次 poll 同步 connect + 写请求（连接建立/写通常快），随后
// 非阻塞读响应——EAGAIN 时写 `cx.fd`（POLLIN）挂起（W3 事件驱动），数据到达
// 累积至头部终止符 `\r\n\r\n` 后 `Ready`（状态码 + body，MVP 不精确按
// Content-Length，读至头部完成即返回）。`block_on` 驱动为主路径。
struct GetAsync {
    fd: i64,
    host: String,
    port: i64,
    path: String,
    req: String,
    buf: String,
    state: i64,
}

impl Future for GetAsync {
    type Output = net::http::Response;
    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output> {
        if self.state == 0 {
            let oct = match ipv4_octets(self.host) {
                Result::Ok(x) => x,
                Result::Err(_) => return Poll::Ready(net::http::Response { status: 0, body: String::new() }),
            };
            match tcp_connect(self.port, oct.a, oct.b, oct.c, oct.d) {
                Result::Ok(f) => self.fd = f,
                Result::Err(_) => return Poll::Ready(net::http::Response { status: 0, body: String::new() }),
            };
            let _ = io::nio::set_nonblocking(self.fd, true);
            let stream = net::tcp::TcpStream { fd: self.fd, addr: net::addr::SocketAddr { ip: self.host, port: 0 } };
            let _ = stream.write(self.req);
            self.state = 1;
        }
        if self.state == 1 {
            match net::recv_some(self.fd, 1024) {
                Result::Ok(s) => {
                    if s.len == 0 {
                        self.state = 2;
                        return Poll::Ready(net::http::Response { status: net::http::parse_status(self.buf), body: net::http::extract_body(self.buf) });
                    }
                    self.buf = self.buf + s;
                    let sep = net::http::find_header_end(self.buf);
                    if sep >= 0 {
                        let status = net::http::parse_status(self.buf);
                        let body = net::http::extract_body(self.buf);
                        self.state = 2;
                        return Poll::Ready(net::http::Response { status: status, body: body });
                    }
                }
                Result::Err(_) => {
                    // EAGAIN / 未就绪 → 挂起等读就绪
                    cx.fd = self.fd;
                    cx.interest = 1;
                    return Poll::Pending;
                }
            }
        }
        Poll::Pending
    }
}

impl HttpClient {
    // W5：异步 GET——返回 `GetAsync` future（`block_on(&mut f)` 驱动或 async fn await）。
    // 简化真异步：connect/写同步，读响应经 wait_fd 挂起（不阻塞线程）。
    fn get_async(&mut self, url: String) -> net::http::GetAsync {
        let u = match net::http::parse_url(url) {
            Result::Ok(x) => x,
            Result::Err(_) => return net::http::GetAsync { fd: -1, host: String::new(), port: 0, path: String::new(), req: String::new(), buf: String::new(), state: 2 },
        };
        let req = String::from("GET ") + u.path + String::from(" HTTP/1.1\r\nHost: ") + u.host + String::from("\r\nConnection: close\r\n\r\n");
        net::http::GetAsync {
            fd: -1,
            host: u.host,
            port: u.port,
            path: u.path,
            req: req,
            buf: String::new(),
            state: 0,
        }
    }
    // W5：异步 POST——返回 `GetAsync` future（复用读响应异步逻辑，请求文本带
    // body + Content-Length），`block_on` 驱动得 `Response`。连接/写同步，读经
    // wait_fd 挂起。
    fn post_async(&mut self, url: String, body: String) -> net::http::GetAsync {
        let u = match net::http::parse_url(url) {
            Result::Ok(x) => x,
            Result::Err(_) => return net::http::GetAsync { fd: -1, host: String::new(), port: 0, path: String::new(), req: String::new(), buf: String::new(), state: 2 },
        };
        let req = String::from("POST ") + u.path + String::from(" HTTP/1.1\r\nHost: ") + u.host + String::from("\r\nContent-Length: ") + int_to_string(body.len) + String::from("\r\nConnection: close\r\n\r\n") + body;
        net::http::GetAsync {
            fd: -1,
            host: u.host,
            port: u.port,
            path: u.path,
            req: req,
            buf: String::new(),
            state: 0,
        }
    }
}
