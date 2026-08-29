// P2 验收：MutexGuard 作用域守卫自动解锁（rlyeh-desugar/src/guard.rs 块尾注入）
// 验证策略：guard 块结束后 try_lock 应成功（输出 1）；guard 持锁期间 try_lock 返回 false（输出 0）。
// try_lock 非阻塞，避免同一线程非递归锁重入死锁。
// 输出与 mutex_guard.out 精确对比
fn main() {
    // A. 块表达式级守卫：块尾自动注入 g.unlock()
    let mut m = Mutex::new(0);
    {                                // 块表达式语句须加分号（parse_block：无分号即块尾表达式）
        let g = m.lock_guard();      // 加锁
        let t = m.try_lock();        // 守卫持锁中 → false
        if t { println(1) } else { println(0) }   // 0
    };                               // ← 自动注入 g.unlock()
    let ok = m.try_lock();           // 已解锁 → true
    if ok { println(1) } else { println(0) }      // 1
    if ok { m.unlock(); }

    // B. 嵌套块（if 分支）守卫：分支块尾注入（与 Rust 词法作用域一致）
    let mut m2 = Mutex::new(0);
    let r = if true {
        let g = m2.lock_guard();
        42
    };                               // ← 自动注入 g.unlock()
    println(r);                      // 42
    let ok2 = m2.try_lock();         // true
    if ok2 { println(1) } else { println(0) }     // 1
    if ok2 { m2.unlock(); }

    // C. 多守卫顺序注入（g1、g2 在块尾依次 unlock）
    let mut m3 = Mutex::new(0);
    let mut m4 = Mutex::new(0);
    {                                // 块表达式语句须加分号
        let g1 = m3.lock_guard();
        let g2 = m4.lock_guard();
    };                               // ← 注入 g1.unlock(); g2.unlock();
    let a = m3.try_lock();
    let b = m4.try_lock();
    if a && b { println(1) } else { println(0) }  // 1（两锁均已释放）
    if a { m3.unlock(); }
    if b { m4.unlock(); }

    // D. while 循环体内守卫：每次迭代各自解锁
    let mut m5 = Mutex::new(0);
    let mut i = 0;
    while i < 3 {
        let g = m5.lock_guard();     // 每次迭代加锁
        i += 1;
    }                                // ← 每次迭代尾自动注入 g.unlock()
    let ok5 = m5.try_lock();         // true（末次迭代后已解锁）
    if ok5 { println(1) } else { println(0) }     // 1
    if ok5 { m5.unlock(); }
}
