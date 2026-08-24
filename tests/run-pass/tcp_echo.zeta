// O1b/O1c/O2：TCP 自连接 echo（免外网）——
// bind(端口 0) → local_addr()（getsockname）读实际端口 → connect → accept → 双向收发
// 失败分支用 `return` 臂静默退出（MVP 无 diverging 块；本地 bind 几乎必然成功）。
fn main() {
    let listener = match TcpListener::bind(SocketAddr::new(String::from("127.0.0.1"), 0)) {
        Result::Ok(l) => l,
        Result::Err(e) => return,
    };
    let local = match listener.local_addr() {
        Result::Ok(s) => s,
        Result::Err(e) => return,
    };
    println(local.port() > 0);   // 端口 0 自动分配 → 实际端口非 0
    let client = match TcpStream::connect(SocketAddr::new(String::from("127.0.0.1"), local.port())) {
        Result::Ok(s) => s,
        Result::Err(e) => return,
    };
    let server = match listener.accept() {
        Result::Ok(s) => s,
        Result::Err(e) => return,
    };
    // client → server 发送 "hello"，server 读回
    let _ = client.write(String::from("hello"));
    let got = match server.read(1024) {
        Result::Ok(s) => s,
        Result::Err(e) => String::from("read-fail"),
    };
    println(got);
    // echo 回 client
    let _ = server.write(got);
    let echoed = match client.read(64) {
        Result::Ok(s) => s,
        Result::Err(e) => String::from("echo-fail"),
    };
    println(echoed);
    // O2 read_line 逐行（\n 与 \r\n 均剥离）
    let _ = client.write(String::from("line1\nline2\r\n"));
    let l1 = match server.read_line() {
        Result::Ok(s) => s,
        Result::Err(e) => String::from("rl-fail"),
    };
    println(l1);
    let l2 = match server.read_line() {
        Result::Ok(s) => s,
        Result::Err(e) => String::from("rl-fail"),
    };
    println(l2);
    // shutdown + peer_addr
    let _ = client.shutdown(Shutdown::Both);
    println(server.peer_addr().port() > 0);
}
