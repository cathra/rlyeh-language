// O1a：SocketAddr / Ipv4Octets / ipv4_octets（纯字符串解析，无网络）
fn main() {
    let a = SocketAddr::new(String::from("127.0.0.1"), 8080);
    println(a.ip());
    println(a.port());
    let p = match SocketAddr::parse(String::from("10.1.2.3:9999")) {
        Result::Ok(s) => s,
        Result::Err(e) => SocketAddr::new(String::from("0.0.0.0"), 0),
    };
    println(p.ip());
    println(p.port());
    // 非法（无冒号）→ Err → 回退
    let bad = match SocketAddr::parse(String::from("nope")) {
        Result::Ok(s) => s,
        Result::Err(e) => SocketAddr::new(String::from("0.0.0.0"), 0),
    };
    println(bad.ip());
    println(bad.port());
    // 无端口（IPv4 仅 IP）→ Err → 回退
    let d = match SocketAddr::parse(String::from("8.8.8.8")) {
        Result::Ok(s) => s,
        Result::Err(e) => SocketAddr::new(String::from("0.0.0.0"), 0),
    };
    println(d.ip());
    println(d.port());
    // ipv4_octets 四段：192+168+0+1 = 361
    let o = match ipv4_octets(String::from("192.168.0.1")) {
        Result::Ok(x) => x,
        Result::Err(e) => Ipv4Octets { a: 0, b: 0, c: 0, d: 0 },
    };
    println(o.a + o.b + o.c + o.d);
    // 非法 IP（少一段）→ Err
    let bad2 = match ipv4_octets(String::from("1.2.3")) {
        Result::Ok(x) => 1,
        Result::Err(e) => 0,
    };
    println(bad2);
}
