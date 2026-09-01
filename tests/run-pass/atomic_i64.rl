// H-M2（SH-P0-4）：`AtomicI64` 原子类型——读改写 / CAS / 内存序分派。
// 底层为 driver 注入的 LLVM `atomicrmw` / `cmpxchg`（SeqCst）。
fn main() {
    let a = AtomicI64::new(10);

    // 载入 / 存储
    println(a.load());                  // 10
    a.store(20);
    println(a.load());                  // 20

    // 读改写族：返回**旧值**
    println(a.fetch_add(5));            // 20（旧值）
    println(a.load());                  // 25
    println(a.fetch_sub(3));            // 25
    println(a.load());                  // 22
    println(a.swap(100));               // 22（旧值）
    println(a.load());                  // 100

    // 位运算族（100 = 0b1100100）
    println(a.fetch_or(7));             // 100（旧值）
    println(a.load());                  // 103（100 | 7 = 0b1100111）
    println(a.fetch_and(15));           // 103（旧值）
    println(a.load());                  // 7（103 & 15 = 0b0000111）
    println(a.fetch_xor(3));            // 7（旧值）
    println(a.load());                  // 4（7 ^ 3）

    // 比较交换：成功 / 失败
    let ok = a.compare_exchange(4, 99);
    println(ok);                        // true（当前 4 == expected）
    println(a.load());                  // 99
    let bad = a.compare_exchange(4, 0);
    println(bad);                       // false（当前 99 != 4）
    println(a.load());                  // 99（未被改写）
    println(a.compare_and_swap(99, 7)); // 99（旧值）
    println(a.load());                  // 7

    // 内存序分派：load_with / store_with
    a.store_with(42, sync::Ordering::Release);
    println(a.load_with(sync::Ordering::Acquire));   // 42
    a.store_with(43, sync::Ordering::Relaxed);
    println(a.load_with(sync::Ordering::Relaxed));   // 43
    a.store_with(44, sync::Ordering::SeqCst);
    println(a.load_with(sync::Ordering::SeqCst));    // 44
}
