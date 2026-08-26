// W6 边界：跨线程闭包限单参数，多参数应报 Unsupported
fn main() {
    let base = 40;
    let f = |x: i64, y: i64| x + y + base;
    let t = Thread::start(f, 2);
    match t {
        Ok(th) => println(th.join()),
        Err(_) => println(-1),
    }
}
