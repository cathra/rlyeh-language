// sync/mutex.rl：互斥锁与守卫（P2/P5，2026-08）
// ===== MutexGuard（P2，2026-08）：作用域守卫自动解锁 =====
// `let g = m.lock_guard();` 后编译器在所在块尾自动注入 `g.unlock();`
// （desugar 阶段对方法名 `lock_guard` 特判，见 rlyeh-desugar/src/guard.rs；
// 注入覆盖所在块 stmts 末尾，final_expr 之前）。
// MVP 限制：if/match 分支内的提前 return / break 不注入（块尾注入前置，
// 显式 `g.unlock()` 手动调用仍可用）；按方法名 `lock_guard` 特判。
// 定义前置：typecheck 对 struct 类型注册顺序敏感（即注册即查），
// 故守卫类型须在 Mutex::lock_guard 引用前声明；字段为锁指针 + 数据引用（无类型依赖），
// unlock 直接经 extern（避免 Mutex 方法前向依赖）。
// P5（2026-08-29 升级为完整泛型）：`MutexGuard<T>` 持 `value: &mut T` 引用被锁值，
// `get`/`get_mut` 经引用读写。（此前「泛型 struct 引用字段构造」障碍已由 P7a 的
// `unify` 复合/引用字段推断修复——`Guard { value: &mut self.value }` 可推断 T。）
struct MutexGuard<T> { p: i64, value: &mut T }

impl<T> MutexGuard<T> {
    // 显式解锁（编译器注入自动调用；手动调用后守卫仍会被再次注入——宽松语义）
    fn unlock(self) {
        let _ = pthread_mutex_unlock(self.p);
    }
    // 读被锁值（解引用守卫持有的引用）
    fn get(&self) -> &T {
        self.value
    }
    // Deref 分发：`*g` 生成 `g.deref()` 返回 T（Copy 类型按值拷贝，约定同 y4b_deref）
    fn deref(&self) -> T {
        *self.value
    }
    // 写被锁值（可变引用写回原 Mutex.value）
    fn get_mut(&mut self) -> &mut T {
        self.value
    }
    // Y4b-4（2026-08-30）：DerefMut 分发——`*g = x` 经 `g.deref_mut()` 返回 &mut T
    fn deref_mut(&mut self) -> &mut T {
        self.value
    }
}

// 互斥锁（非递归；p 为 pthread_mutex_t*）。
// macOS pthread_mutex_t = 64 字节，Linux glibc = 40 字节，calloc(1, 64) 双平台安全。
// P5（2026-08-29）：`Mutex<T>` 带值锁——`value: T` 数据载荷，`new(v: T)` 初始化。
struct Mutex<T> { p: i64, value: T }

impl<T> Mutex<T> {
    fn new(v: T) -> sync::Mutex<T> {
        let p = calloc(1, 64);
        let _ = pthread_mutex_init(p, 0);   // attr = NULL
        sync::Mutex { p: p, value: v }
    }
    // 加锁（无竞争者时立即返回；已持锁线程重复加锁为未定义行为）
    fn lock(&self) {
        let _ = pthread_mutex_lock(self.p);
    }
    fn unlock(&self) {
        let _ = pthread_mutex_unlock(self.p);
    }
    // 尝试加锁：成功返回 true，已被占用返回 false（不阻塞）
    fn try_lock(&self) -> bool {
        let r = pthread_mutex_trylock(self.p);
        r == 0
    }
    // P2：lock_guard 返回守卫（加锁并返回 MutexGuard；作用域结束自动解锁）。
    // P5：`&mut self` 借用调用方 Mutex，`value: &mut self.value` 指向调用方值
    // （规避值传递参数拷贝后 `&value` 悬垂）。
    fn lock_guard(&mut self) -> sync::MutexGuard<T> {
        self.lock();
        sync::MutexGuard { p: self.p, value: &mut self.value }
    }
}
