// F-M3：Thread::start 闭包捕获借用引用（指向外层栈帧）属非 'static，应拒绝
// expect: static
fn main() {
    let x = 5;
    let r = &x;
    let t = Thread::start(move || *r);
    match t {
        Ok(th) => println(th.join()),
        Err(_) => println(-1),
    }
}
