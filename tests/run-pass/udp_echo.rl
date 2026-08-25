// Y7（2026-08）：UDP 自回环（bind(0) 内核分配端口 → send_to 自身 → recv_from 回显）。
// 输出确定：发送字节数 8 + 报文原文 + 源端口有效位（端口随机不打印）。
// 注：`let n = match .. { Ok(x) => x }` 的标量 Ok 绑定受 LIR 限制（期望 ptr），
// 用语句 match 直接消费（聚合类型绑定不受限）。
fn main() {
    let addr = SocketAddr::new(String::from("127.0.0.1"), 0);
    let s = match UdpSocket::bind(addr) {
        Result::Ok(x) => x,
        Result::Err(e) => return,
    };
    let target = SocketAddr::new(String::from("127.0.0.1"), s.local_addr().port());
    match s.send_to(String::from("rlyeh-udp"), target) {
        Result::Ok(n) => println(n),
        Result::Err(e) => return,
    }
    match s.recv_from(64) {
        Result::Ok(p) => {
            println(p.data);
            println(p.from.port() > 0);
        }
        Result::Err(e) => return,
    }
}
