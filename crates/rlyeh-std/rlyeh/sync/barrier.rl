// sync/barrier.rl：屏障（P 阶段，2026-08）
// 屏障（p 为 pthread_barrier_t*；macOS = 64 字节、Linux = 32 字节，calloc(1, 64) 双平台安全）。
// 阻塞至 count 个线程到达后同时放行；MVP 统一返回 0
// （PTHREAD_BARRIER_SERIAL_THREAD = -1 领头线程语义简化）。
struct Barrier { p: i64 }

impl Barrier {
    fn new(count: i64) -> sync::Barrier {
        let p = calloc(1, 64);
        let _ = pthread_barrier_init(p, 0, count);
        sync::Barrier { p: p }
    }
    fn wait(self) -> i64 {
        let _ = pthread_barrier_wait(self.p);
        0
    }
}
