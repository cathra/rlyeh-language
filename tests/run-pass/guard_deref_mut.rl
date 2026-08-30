// Y4b-4：guard DerefMut 分发——`*g = x` 经 `deref_mut` 写回被锁值，作用域自动解锁。
fn main() {
    // MutexGuard DerefMut
    let mut m = Mutex::new(42);
    {
        let mut g = m.lock_guard();
        *g = 99;            // DerefMut 写回
        println(*g);        // 99
    };
    println(m.value);       // 99（写回生效）

    // RwLockWriteGuard DerefMut
    let mut rw = RwLock::new(7);
    {
        let mut wg = rw.write_guard();
        *wg = 33;           // DerefMut 写回
        println(*wg);       // 33
    };
    println(0);
}
