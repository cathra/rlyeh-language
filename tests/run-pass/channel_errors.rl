// Y4c（2026-08-30）：通道错误类型 SendError/RecvError/TryRecvError（Result 语义，替代 Option 退化）
// 既有 recv/try_recv/send（Option/()）便捷 API 保留兼容；错误类型经 Result 方法暴露
fn main() {
    // 1. try_recv_result：空队列 → Err(TryRecvError{kind:0})
    let mut pair = channel::<i64>();
    let mut tx = pair.tx;
    let mut rx = pair.rx;
    match rx.try_recv_result() {
        Result::Ok(v) => println(v),
        Result::Err(e) => println(e.kind),    // 0（空）
    }
    // 发送后 → Ok(42)
    tx.send(42);
    match rx.try_recv_result() {
        Result::Ok(v) => println(v),          // 42
        Result::Err(e) => println(e.kind),
    }
    // 再取（空）→ 0
    match rx.try_recv_result() {
        Result::Ok(v) => println(v),
        Result::Err(e) => println(e.kind),    // 0（空）
    }
    // close 后 → Err(kind:1 断开)
    tx.close();
    match rx.try_recv_result() {
        Result::Ok(v) => println(v),
        Result::Err(e) => println(e.kind),    // 1（断开）
    }

    // 2. recv_result：close 且空 → Err(RecvError)
    let mut pair2 = channel::<i64>();
    let mut rx2 = pair2.rx;
    pair2.tx.close();
    match rx2.recv_result() {
        Result::Ok(v) => println(v),
        Result::Err(_e) => println(-1),        // -1（断开且空）
    }

    // 3. send_result：关闭通道后 send → Err(SendError)
    let mut pair3 = channel::<i64>();
    pair3.tx.close();
    match pair3.tx.send_result(7) {
        Result::Ok(_) => println(1),
        Result::Err(_e) => println(-1),        // -1（已关闭）
    }

    println(0);
}
