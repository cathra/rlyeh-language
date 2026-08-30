// Y4b-3：RwLock<T> 带值锁 + 读/写守卫（作用域自动解锁）+ Deref 分发
// `read_guard`/`write_guard` 返回守卫，块尾由 desugar 注入 `g.unlock()`；
// `*g` 经 Deref 分发生成 `g.deref()` 返回 T（Copy 按值拷贝）。
fn main() {
    let mut rw = RwLock::new(42);
    // 1. 读守卫：*g 经 Deref 返回 42
    {
        let g = rw.read_guard();
        println(*g);          // 42
    };
    // 2. 写守卫：get_mut 改值（块尾自动解锁）
    {
        let mut wg = rw.write_guard();
        *wg.get_mut() = 77;
        println(*wg);         // 77
    };
    // 3. 读守卫读回写后值 + get()
    {
        let g = rw.read_guard();
        println(*g);          // 77
        println(*g.get());    // 77
    };
    println(0);
}
