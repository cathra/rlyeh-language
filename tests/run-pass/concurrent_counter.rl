// H-M4（SH-P0-4）：并发计数器——多线程 + `Arc<AtomicI64>` / `Arc<Mutex<i64>>` 无数据竞争。
// 两个线程各对共享计数器自增 2000 次：
// - 原子路径：`fetch_add`（driver 注入的 LLVM `atomicrmw` SeqCst）无锁、无丢失更新；
// - 锁路径：`Arc<Mutex<i64>>` 经 `lock_guard` 作用域守卫串行化临界区。
// 两路径最终计数均须恰为 4000——若存在数据竞争（丢失更新）结果会偏小。
fn main() -> i64 {
    let mut atom = Arc::new(AtomicI64::new(0));
    let mut mtx = Arc::new(Mutex::new(0));

    let a1 = atom.clone();
    let a2 = atom.clone();
    let m1 = mtx.clone();
    let m2 = mtx.clone();

    let t1 = Thread::start(move || {
        let mut i = 0;
        while i < 2000 {
            a1.fetch_add(1);
            let mut g = m1.lock_guard();
            let p = (&mut g).get_mut();
            *p = *p + 1;
            i = i + 1;
        }
        0
    });
    let t2 = Thread::start(move || {
        let mut i = 0;
        while i < 2000 {
            a2.fetch_add(1);
            let mut g = m2.lock_guard();
            let p = (&mut g).get_mut();
            *p = *p + 1;
            i = i + 1;
        }
        0
    });

    match t1 {
        Ok(th1) => match t2 {
            Ok(th2) => {
                let _ = th1.join();
                let _ = th2.join();
                println(atom.load());          // 4000（原子自增无丢失）
                let g = mtx.lock_guard();
                let v = g.get();
                println(*v);                   // 4000（锁保护无丢失）
                if atom.load() == 4000 && *v == 4000 {
                    println(1);                // 1：无数据竞争
                } else {
                    println(0);
                }
                0
            }
            Err(_) => { println(-2); 1 }
        },
        Err(_) => { println(-1); 1 }
    }
}
