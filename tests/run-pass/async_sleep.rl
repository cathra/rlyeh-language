// W3：事件驱动 future::sleep（async fn 内 await，定时器休眠）
async fn g() -> i64 {
    let s: future::Sleep = future::sleep(Duration::milliseconds(20));
    s.await;
    42
}

fn main() {
    let mut f = g();
    let v = block_on(&mut f);
    println(v);                    // 42
}
