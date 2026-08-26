// 探针：闭包跨线程捕获（W6）——Thread::start(f, arg) 带参闭包值 + 输入参数
fn main() {
    // 单捕获
    let base = 40;
    let f1 = |x: i64| x + base;
    let t1 = Thread::start(f1, 2);
    match t1 {
        Ok(th) => println(th.join()),
        Err(_) => println(-1),
    }
    // 多捕获
    let ca = 30;
    let cb = 12;
    let f2 = |x: i64| x + ca + cb;
    let t2 = Thread::start(f2, 0);
    match t2 {
        Ok(th) => println(th.join()),
        Err(_) => println(-1),
    }
    // String 捕获
    let s = String::from("hello");
    let f3 = |x: i64| x + s.len();
    let t3 = Thread::start(f3, 37);
    match t3 {
        Ok(th) => println(th.join()),
        Err(_) => println(-1),
    }
}
