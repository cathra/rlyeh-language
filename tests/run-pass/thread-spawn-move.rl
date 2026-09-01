// F-M4：Thread::start(move || ...) 跨线程执行（等价 actor-runtime worker_loop）
// + F-M2：move 闭包捕获拥有环境（跨线程所有权转移）
// + F-M3：'static 约束（捕获标量 + 堆拥有数据 String，无借用引用）
// 注：`spawn` 为 actor 派生保留关键字，线程启动统一用 `Thread::start`。
fn main() -> i64 {
    let a = 10;
    let b = 20;
    let s = String::from("xy");   // len 2，拥有数据（堆，'static 安全）
    // 两个 move 闭包，分别捕获标量 / 拥有堆数据，跨线程执行后 join 校验
    let t1 = Thread::start(move || a + 5);                    // → 15
    let t2 = Thread::start(move || { let n = s.len() + b; n });  // 2 + 20 = 22
    match t1 {
        Ok(th1) => match t2 {
            Ok(th2) => {
                let r1 = th1.join();
                let r2 = th2.join();
                if r1 == 15 && r2 == 22 {
                    println("spawn-ok");
                    0
                } else {
                    println("spawn-bad");
                    1
                }
            }
            Err(_) => { println("spawn-fail-2"); 1 }
        },
        Err(_) => { println("spawn-fail-1"); 1 }
    }
}
