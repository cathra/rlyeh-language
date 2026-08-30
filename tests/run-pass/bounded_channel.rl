// Y4c：有界通道 bounded_channel(capacity)——容量限制 + 满则 send 阻塞（condvar 挂起）
// 单线程功能验证：try_send 满返回 false、recv FIFO、腾出空间后恢复 send
fn main() {
    let mut pair = sync::bounded_channel::<i64>(2);
    let mut tx = pair.tx;
    let mut rx = pair.rx;
    // 容量 2：发送 2 个不阻塞
    tx.send(10);
    tx.send(20);
    // 第 3 个若阻塞需消费者；用 try_send 验证"满"判定（不阻塞，返回 false）
    if tx.try_send(30) {
        println(1);          // 不应到达
    } else {
        println(0);          // 0（满）
    }
    // 消费腾出空间
    match rx.recv() {
        Option::Some(v) => println(v),   // 10
        Option::None => println(-1),
    }
    // 现在 try_send 成功
    if tx.try_send(30) {
        println(1);          // 1
    } else {
        println(0);
    }
    match rx.recv() {
        Option::Some(v) => println(v),   // 20
        Option::None => println(-1),
    }
    match rx.recv() {
        Option::Some(v) => println(v),   // 30
        Option::None => println(-1),
    }
    println(0);              // 0（结束）
}
