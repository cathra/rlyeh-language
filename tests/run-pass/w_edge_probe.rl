// W 收尾边界：递归 async + 嵌套 + 多函数环 + 递归跨线程 + fn/闭包双入口共存
fn worker() -> i64 {
    7
}
async fn countdown(n: i64) -> i64 {
    if n == 0 {
        return 0;
    }
    let r = countdown(n - 1).await;
    r + 1
}
async fn helper(n: i64) -> i64 {
    n * 2
}
async fn mix(n: i64) -> i64 {
    if n == 0 {
        return 0;
    }
    let h = helper(n).await;
    let r = mix(n - 1).await;
    r + h
}
async fn ping(x: i64) -> i64 {
    if x <= 0 {
        return 100;
    }
    let r = pong(x - 1).await;
    r + 1
}
async fn pong(x: i64) -> i64 {
    let r = ping(x - 1).await;
    r + 2
}
fn main() {
    // 常规 fn() 入口（与闭包跨线程共存）
    let t0 = Thread::start(worker);
    match t0 {
        Ok(th) => println(th.join()),
        Err(_) => println(-1),
    }
    let mut f1 = countdown(5);
    println(block_on(&mut f1));
    let mut f2 = mix(3);
    println(block_on(&mut f2));
    let mut f3 = ping(2);
    println(block_on(&mut f3));
    // 递归 async 结果作为跨线程闭包捕获输入
    let mut f4 = countdown(4);
    let cd = block_on(&mut f4);
    let thread_fn = |x: i64| x + cd;
    let t = Thread::start(thread_fn, 1);
    match t {
        Ok(th) => println(th.join()),
        Err(_) => println(-1),
    }
}
