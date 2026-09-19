// sync/rwlock.rl：读写锁与读/写守卫（Y4b-3，2026-08）
// 读写锁（读-读共享，写-写/读-写互斥；p 为 pthread_rwlock_t*）。
// macOS pthread_rwlock_t = 200 字节（__opaque[192] + __sig），Linux glibc = 56 字节，
// calloc(1, 256) 双平台安全。
// Y4b-3（2026-08-30）：`RwLock<T>` 带值锁——`value: T` 数据载荷，`new(v: T)` 初始化；
// `read_guard`/`write_guard` 返回读/写守卫（作用域结束自动解锁，desugar 特判
// read_guard/write_guard，见 rlyeh-desugar/src/guard.rs），`*guard` 经 Deref 分发
// （typecheck UnaryOp::Deref 查 guard 的 `deref` 方法，返回 T 拷贝，约定同 y4b_deref）。
struct RwLock<T> { p: i64, value: T }

impl<T> RwLock<T> {
    fn new(v: T) -> sync::RwLock<T> {
        let p = calloc(1, 256);
        let _ = pthread_rwlock_init(p, 0);
        sync::RwLock { p: p, value: v }
    }
    // 读锁（可多个读者并发持有）
    fn read_lock(&self) {
        let _ = pthread_rwlock_rdlock(self.p);
    }
    // 写锁（与任何其他锁互斥）
    fn write_lock(&self) {
        let _ = pthread_rwlock_wrlock(self.p);
    }
    fn unlock(&self) {
        let _ = pthread_rwlock_unlock(self.p);
    }
    fn try_read_lock(&self) -> bool {
        let r = pthread_rwlock_tryrdlock(self.p);
        r == 0
    }
    fn try_write_lock(&self) -> bool {
        let r = pthread_rwlock_trywrlock(self.p);
        r == 0
    }
    // Y4b-3：读守卫（作用域结束自动解锁）
    fn read_guard(&self) -> sync::RwLockReadGuard<T> {
        self.read_lock();
        sync::RwLockReadGuard { p: self.p, value: &self.value }
    }
    // Y4b-3：写守卫（作用域结束自动解锁）
    fn write_guard(&mut self) -> sync::RwLockWriteGuard<T> {
        self.write_lock();
        sync::RwLockWriteGuard { p: self.p, value: &mut self.value }
    }
}

// Y4b-3：读守卫——`value: &T` 被锁值引用，`get`/`deref` 经引用读。
struct RwLockReadGuard<T> { p: i64, value: &T }

impl<T> RwLockReadGuard<T> {
    fn unlock(self) {
        let _ = pthread_rwlock_unlock(self.p);
    }
    fn get(&self) -> &T {
        self.value
    }
    // Deref 分发：`*g` 生成 `g.deref()` 返回 T（Copy 类型按值拷贝）
    fn deref(&self) -> T {
        *self.value
    }
}

// Y4b-3：写守卫——`value: &mut T` 被锁值可变引用，`get`/`get_mut` 读写。
struct RwLockWriteGuard<T> { p: i64, value: &mut T }

impl<T> RwLockWriteGuard<T> {
    fn unlock(self) {
        let _ = pthread_rwlock_unlock(self.p);
    }
    fn get(&self) -> &T {
        self.value
    }
    fn get_mut(&mut self) -> &mut T {
        self.value
    }
    // Deref 分发：`*g` 生成 `g.deref()` 返回 T（Copy 类型按值拷贝）
    fn deref(&self) -> T {
        *self.value
    }
    // Y4b-4（2026-08-30）：DerefMut 分发——`*g = x` 经 `g.deref_mut()` 返回 &mut T
    fn deref_mut(&mut self) -> &mut T {
        self.value
    }
}
