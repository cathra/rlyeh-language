// sync/condvar.rl：条件变量（P 阶段，2026-08）
// ===== P 阶段（2026-08）：条件变量 / 屏障 / 并发通道 =====
// 线程创建已支持（S0 ✅，driver 注入 __rlyeh_thread_spawn），
// wait/signal 协作与屏障的 count>1 语义可在多线程下验证。

// 条件变量（p 为 pthread_cond_t*；macOS = 40 字节、Linux = 48 字节，calloc(1, 64) 双平台安全）。
// 与 Mutex 配对使用：wait 原子地释放 m 并阻塞，被 notify 唤醒后重新获取 m 返回。
struct Condvar { p: i64 }

impl Condvar {
    fn new() -> sync::Condvar {
        let p = calloc(1, 64);
        let _ = pthread_cond_init(p, 0);   // attr = NULL
        sync::Condvar { p: p }
    }
    // 调用方须已持有 m（与同一 Mutex 配对；伪唤醒由调用方循环复查条件）
    // P5：Mutex 泛型化，wait 参数接受任意 Mutex<T>（锁原语只依赖 p 字段）。
    fn wait<T>(self, m: sync::Mutex<T>) {
        let _ = pthread_cond_wait(self.p, m.p);
    }
    // 唤醒一个等待者（无等待者时空操作）
    fn notify_one(self) {
        let _ = pthread_cond_signal(self.p);
    }
    // 唤醒全部等待者
    fn notify_all(self) {
        let _ = pthread_cond_broadcast(self.p);
    }
}
