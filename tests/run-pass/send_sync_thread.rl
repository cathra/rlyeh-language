// SH-P3-1 L1（M1+M2+M3）：跨线程经 `Thread::start(move || ...)` 共享 `Arc<Mutex<i64>>`，
// 满足 Send + Sync 并发安全基线（告警式检查通过，不触发 W002）。
// 单线程自增一次，确定性输出 `1`。
fn main() -> i64 {
    let counter = Arc::new(Mutex::new(0));
    let c = counter.clone();
    let t = Thread::start(move || {
        let mut g = c.lock_guard();
        let p = (&mut g).get_mut();
        *p = *p + 1;
        0
    });
    match t {
        Ok(th) => {
            let _ = th.join();
            let g = counter.lock_guard();
            println(*g.get());   // 1
            0
        }
        Err(_) => { println(-1); 1 }
    }
}
