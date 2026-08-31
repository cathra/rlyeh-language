# 网络（net 模块）

TCP / HTTP / UDP 客户端与服务端 API，位于 `net` 子模块。WASI 目标下网络禁用（L4 ✅）。

> **C 程序员对照**：Rlyeh 的 `TcpStream`/`TcpListener`/`UdpSocket` ≈ C 的 Berkeley sockets（`socket`/`connect`/`bind`/`listen`/`accept`/`recv`/`send`），但**对象化 + 自动关闭**，且 `read`/`write` 直接收 `&[u8]` 切片（带长度，不用你再传 `size`/`count` 参数，也不易溢出）。`HttpClient` 则相当于 libcurl 的简化版——一行发起 HTTP 请求拿 `Response`。

## SocketAddr

### 构造

```rlyeh
let addr = SocketAddr::from_ip_port(String::from("127.0.0.1"), 8080);
```

- `SocketAddr::from_ip_port(ip: &str, port: i64) -> SocketAddr`

### 成员

| 成员 | 类型 | 说明 |
|------|------|------|
| `ip` | `String` | IP 地址字符串 |
| `port` | `i64` | 端口 |

```rlyeh
println(addr.ip);
println(addr.port);
```

## TcpStream

### 构造

```rlyeh
let stream = TcpStream::connect(addr);          // 连接到 SocketAddr
let s2 = TcpStream::from_raw_fd(5);             // 从 fd 接管
```

### 方法

| 方法 | 签名 | 说明 |
|------|------|------|
| `read` | `(buf: &mut [u8]) -> i64` | 读入缓冲，返回字节数（EOF 为 0） |
| `write` | `(buf: &[u8]) -> i64` | 写出，返回字节数 |
| `is_eof` | `() -> bool` | 是否到流尾 |
| `set_nonblocking` | `(b: bool) -> ()` | 设置为非阻塞（NIO 用） |
| `shutdown` | `() -> ()` | 关闭读写 |
| `peer_addr` | `() -> SocketAddr` | 对端地址 |
| `local_addr` | `() -> SocketAddr` | 本地地址 |
| `into_raw_fd` | `() -> i64` | 释放 fd 所有权（不关闭） |
| `close` | `() -> ()` | 关闭连接 |

```rlyeh
let stream = TcpStream::connect(SocketAddr::from_ip_port(String::from("example.com"), 80));
let req = String::from("GET / HTTP/1.0\r\n\r\n");
stream.write(req.as_slice());
let mut buf: Vec<u8> = Vec::with_capacity(4096);
let n = stream.read(buf.as_mut_slice());
```

## TcpListener

### 构造

```rlyeh
let listener = TcpListener::bind(addr);
```

### 方法

| 方法 | 签名 | 说明 |
|------|------|------|
| `accept` | `() -> TcpStream` | 阻塞接受连接 |
| `set_nonblocking` | `(b: bool) -> ()` | 非阻塞 |
| `local_addr` | `() -> SocketAddr` | 监听地址 |
| `close` | `() -> ()` | 关闭监听 |

```rlyeh
let listener = TcpListener::bind(SocketAddr::from_ip_port(String::from("0.0.0.0"), 8080));
loop {
    let conn = listener.accept();
    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    conn.read(buf.as_mut_slice());
}
```

## HttpClient

### 方法（或自由函数）

```rlyeh
let resp = HttpClient::get(String::from("https://example.com"));
let resp2 = HttpClient::post(String::from("https://example.com"), String::from("body"));
```

- `HttpClient::get(url: &str) -> Response`
- `HttpClient::post(url: &str, body: &str) -> Response`

> 连接复用（Y3 ✅）：同域名请求复用底层 TCP 连接。

### Response 成员

| 成员 | 类型 | 说明 |
|------|------|------|
| `status` | `i64` | HTTP 状态码 |
| `body` | `String` | 响应体 |
| `headers` | `HashMap<String, String>` | 响应头 |

```rlyeh
println(resp.status);
println(resp.body);
```

## UdpSocket

```rlyeh
let sock = UdpSocket::bind(SocketAddr::from_ip_port(String::from("0.0.0.0"), 53));
let n = sock.send_to(buf, target_addr);         // 发送，返回字节数
let (n, from) = sock.recv_from(buf);            // 接收，返回 (字节数, 来源地址)
```

- `UdpSocket::bind(addr) -> UdpSocket`
- `send_to(buf: &[u8], addr: SocketAddr) -> i64`
- `recv_from(buf: &mut [u8]) -> (i64, SocketAddr)`

## 完整示例（echo 服务端骨架）

```rlyeh
fn main() {
    let addr = SocketAddr::from_ip_port(String::from("127.0.0.1"), 9000);
    let listener = TcpListener::bind(addr);
    let conn = listener.accept();
    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    let n = conn.read(buf.as_mut_slice());
    conn.write(buf.as_slice());
}
```

---

[← 返回标准库详述索引](./index.md)
