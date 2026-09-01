// F-M2：跨线程闭包须为 move（捕获环境所有权转移）。非 move 的捕获闭包
// （borrow）跨线程会共享外层栈帧，应拒绝。
// expect: move
fn main() {
    let base = 1;
    let f = |x: i64| x + base;   // borrow 闭包，捕获 base（非 move）
    let t = Thread::start(f);
    match t {
        Ok(th) => println(th.join()),
        Err(_) => println(-1),
    }
}
