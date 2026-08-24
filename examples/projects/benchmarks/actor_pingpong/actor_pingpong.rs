// 基准: actor_pingpong —— 5 万次线程间同步往返（mpsc 双通道）
// 与 actor_pingpong.zeta 逻辑严格一致。输出 = 1250025000
use std::sync::mpsc;
use std::thread;

const N: i64 = 50000;

fn main() {
    let (tx_a, rx_a) = mpsc::channel::<i64>();
    let (tx_b, rx_b) = mpsc::channel::<i64>();
    let worker = thread::spawn(move || {
        let mut count: i64 = 0;
        for _ in 0..N {
            let _v = rx_a.recv().unwrap();
            count += 1;
            tx_b.send(count).unwrap();
        }
    });
    let mut sum: i64 = 0;
    for i in 0..N {
        tx_a.send(i).unwrap();
        sum += rx_b.recv().unwrap();
    }
    worker.join().unwrap();
    println!("{}", sum);
}
