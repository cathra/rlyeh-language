// W5：Receiver::recv_async 真异步 future（RecvAsync）
// - send 后 block_on 驱动立即 Ready 取到消息
// - close 后空返回 -1（哨兵，Option::None 语义）
fn main() {
    // send 后立即 Ready
    let mut p1 = channel();
    let mut tx1 = p1.tx;
    let mut rx1 = p1.rx;
    tx1.send(7);
    let mut r1: sync::RecvAsync = rx1.recv_async();
    let v1 = block_on(&mut r1);
    println(v1);                    // 7

    // close 且空 → -1
    tx1.close();
    let mut r2: sync::RecvAsync = rx1.recv_async();
    let v2 = block_on(&mut r2);
    println(v2);                    // -1
}
