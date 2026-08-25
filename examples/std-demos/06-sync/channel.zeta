// P1 并发通道：无界队列 send/recv / try_* / close / iter / 多 Sender 共享
// 输出与 channel.out 精确对比
fn main() {
    // 1. 基础 send/recv（FIFO）
    let mut pair = channel();
    let mut tx = pair.tx;
    let mut rx = pair.rx;
    tx.send(10);
    tx.send(20);
    tx.send(30);
    match rx.recv() {
        Option::Some(v) => println(v),   // 10
        Option::None => println(-1),
    }
    match rx.recv() {
        Option::Some(v) => println(v),   // 20
        Option::None => println(-1),
    }

    // 2. try_recv（非空立即返回 / 空返回 None）
    match rx.try_recv() {
        Option::Some(v) => println(v),   // 30
        Option::None => println(-1),
    }
    match rx.try_recv() {
        Option::Some(v) => println(v),
        Option::None => println(0),      // 0（空）
    }

    // 3. try_send 恒成功（无界队列）
    if tx.try_send(99) {
        println(1);                      // 1
    } else {
        println(0);
    }
    match rx.recv() {
        Option::Some(v) => println(v),   // 99
        Option::None => println(-1),
    }

    // 4. close 后队列耗尽 recv → None
    tx.close();
    match rx.recv() {
        Option::Some(v) => println(v),
        Option::None => println(-1),     // -1
    }

    // 5. iter（next 接入 for 循环，不阻塞）
    let mut pair2 = channel();
    let mut tx2 = pair2.tx;
    let mut rx2 = pair2.rx;
    tx2.send(1);
    tx2.send(2);
    tx2.send(3);
    tx2.close();
    let mut sum = 0;
    for v in rx2 {
        sum += v;
    }
    println(sum);                        // 6

    // 6. 多 Sender / Receiver 共享同一队列（Rc clone）
    let mut pair3 = channel();
    let mut tx3a = pair3.tx;
    let mut rx3 = pair3.rx;
    let mut tx3b = tx3a.clone();
    tx3a.send(5);
    tx3b.send(6);
    match rx3.recv() {
        Option::Some(v) => println(v),   // 5
        Option::None => println(-1),
    }
    match rx3.recv() {
        Option::Some(v) => println(v),   // 6
        Option::None => println(-1),
    }

    // 7. recv_async（S3a，MVP 同步语义）：非空立即返回 / close 后空返回 None
    let mut pair4 = channel();
    let mut tx4 = pair4.tx;
    let mut rx4 = pair4.rx;
    tx4.send(7);
    match rx4.recv_async() {
        Option::Some(v) => println(v),   // 7
        Option::None => println(-1),
    }
    tx4.close();
    match rx4.recv_async() {
        Option::Some(v) => println(v),
        Option::None => println(-1),     // -1
    }
}
