// W5：Receiver::recv_async 真异步 future（RecvAsync<T>）
// - send 后 block_on 驱动立即 Ready 取到消息
// - P7c：`Output = Option<T>`——close 后空返回 None（替代 MVP 哨兵 -1）
fn main() {
    // send 后立即 Ready
    let mut p1 = channel::<i64>();
    let mut tx1 = p1.tx;
    let mut rx1 = p1.rx;
    tx1.send(7);
    let mut r1: sync::RecvAsync<i64> = rx1.recv_async();
    match block_on(&mut r1) {
        Option::Some(v) => println(v),  // 7
        Option::None => println(-1),
    }

    // close 且空 → None
    tx1.close();
    let mut r2: sync::RecvAsync<i64> = rx1.recv_async();
    match block_on(&mut r2) {
        Option::Some(v) => println(v),
        Option::None => println(-1),    // close 且空
    }
}
