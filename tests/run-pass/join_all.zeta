// S2b（2026-08）：`thread::join_all` 并发等待多线程 + 墙钟 elapsed。
// - worker 各 sleep 20ms 模拟并发负载；join_all 阻塞至全部完成，
//   按传入顺序返回返回值 [10, 20, 30]，求和 60。
// - `Instant::now/elapsed` 基于墙钟（clock_gettime CLOCK_MONOTONIC，
//   S2b 接入），睡眠期间推进——断言 elapsed >= 20ms（墙钟验证）。
fn w1() -> i64 {
    sleep(Duration::milliseconds(20));
    10
}
fn w2() -> i64 {
    sleep(Duration::milliseconds(20));
    20
}
fn w3() -> i64 {
    sleep(Duration::milliseconds(20));
    30
}

fn main() -> i64 {
    let t1 = Thread::start(w1);
    let t2 = Thread::start(w2);
    let t3 = Thread::start(w3);
    match t1 {
        Ok(a) => match t2 {
            Ok(b) => match t3 {
                Ok(c) => {
                    let mut ts: Vec<thread::Thread> = Vec::with_capacity(3);
                    ts.push(a);
                    ts.push(b);
                    ts.push(c);
                    let t0 = Instant::now();
                    let rs = join_all(ts);
                    let d = t0.elapsed();
                    let sum = rs[0] + rs[1] + rs[2];
                    if sum == 60 && d.millis() >= 20 {
                        println("join-all-ok");
                    } else {
                        println("join-all-fail");
                    }
                }
                Err(_) => println("spawn-fail-3"),
            },
            Err(_) => println("spawn-fail-2"),
        },
        Err(_) => println("spawn-fail-1"),
    }
    0
}
