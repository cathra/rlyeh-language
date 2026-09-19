// sync/atomic.rl：内存序与原子整数（H-M2，2026-09-02）
// ===== H-M2（SH-P0-4，2026-09-02）：内存序 + 原子类型 `AtomicI64` =====
// 底层为 driver 注入的 LLVM 原子指令（core.rl 的 `__rlyeh_atomic_*` extern）：
// 全部 RMW / CAS 固定 SeqCst（最强序，等价 Rust `Ordering::SeqCst`，跨线程
// total order）；`load` / `store` 经 `load_with` / `store_with` 支持
// Relaxed / Acquire / Release / SeqCst 分派。
// MVP 仅提供 i64 字长（`AtomicI64`）；`AtomicBool` / `AtomicUsize` / 指针原子
// 与 `fetch_update` 等派生 API 待后续补齐（对齐 Rust `AtomicI64` 子集）。

// 内存序（取值与 Rust `Ordering` 语义对应；无 AcqRel 的 load/store 语义，
// 分派时 AcqRel 归入 SeqCst）。
enum Ordering {
    Relaxed = 0,
    Acquire = 1,
    Release = 2,
    AcqRel = 3,
    SeqCst = 4,
}

// 原子整数：`p` 为 8 字节对齐缓冲句柄（`calloc(1, 8)`），承载被原子访问的 i64。
struct AtomicI64 { p: i64 }

impl AtomicI64 {
    fn new(v: i64) -> sync::AtomicI64 {
        let p = calloc(1, 8);
        // 初始值须显式写入缓冲（`calloc` 零初始化仅对 `new(0)` 正确）；
        // 对象尚不存在，直接经底层原语存储（无需经 `&self` 方法）。
        let _ = __rlyeh_atomic_store_i64_seq_cst(p, v);
        sync::AtomicI64 { p: p }
    }
    // 载入（SeqCst）
    fn load(&self) -> i64 {
        self.load_with(sync::Ordering::SeqCst)
    }
    // 载入（按内存序分派：Relaxed / Acquire / SeqCst）
    fn load_with(&self, order: sync::Ordering) -> i64 {
        if order == sync::Ordering::Relaxed {
            return __rlyeh_atomic_load_i64_relaxed(self.p);
        }
        if order == sync::Ordering::Acquire {
            return __rlyeh_atomic_load_i64_acquire(self.p);
        }
        __rlyeh_atomic_load_i64_seq_cst(self.p)
    }
    // 存储（SeqCst）
    fn store(&self, v: i64) {
        self.store_with(v, sync::Ordering::SeqCst);
    }
    // 存储（按内存序分派：Relaxed / Release / SeqCst）
    fn store_with(&self, v: i64, order: sync::Ordering) {
        if order == sync::Ordering::Relaxed {
            let _ = __rlyeh_atomic_store_i64_relaxed(self.p, v);
            return;
        }
        if order == sync::Ordering::Release {
            let _ = __rlyeh_atomic_store_i64_release(self.p, v);
            return;
        }
        let _ = __rlyeh_atomic_store_i64_seq_cst(self.p, v);
    }
    // 交换：写入新值并返回旧值（SeqCst）
    fn swap(&self, v: i64) -> i64 {
        __rlyeh_atomic_swap_i64(self.p, v)
    }
    // 读改写族：返回**旧值**（SeqCst）
    fn fetch_add(&self, v: i64) -> i64 {
        __rlyeh_atomic_fetch_add_i64(self.p, v)
    }
    fn fetch_sub(&self, v: i64) -> i64 {
        __rlyeh_atomic_fetch_sub_i64(self.p, v)
    }
    fn fetch_and(&self, v: i64) -> i64 {
        __rlyeh_atomic_fetch_and_i64(self.p, v)
    }
    fn fetch_or(&self, v: i64) -> i64 {
        __rlyeh_atomic_fetch_or_i64(self.p, v)
    }
    fn fetch_xor(&self, v: i64) -> i64 {
        __rlyeh_atomic_fetch_xor_i64(self.p, v)
    }
    // 比较交换：当前值 == expected 时写入 desired。返回旧值——
    // `old == expected` 即交换成功（`compare_and_swap` 语义）。
    fn compare_and_swap(&self, expected: i64, desired: i64) -> i64 {
        __rlyeh_atomic_cas_i64(self.p, expected, desired)
    }
    // 比较交换（成功标识）：成功返回 true，失败返回 false
    // （失败时当前值 != expected，可重读后重试；无 `compare_exchange_weak` 伪失败变体）。
    fn compare_exchange(&self, expected: i64, desired: i64) -> bool {
        let old = __rlyeh_atomic_cas_i64(self.p, expected, desired);
        old == expected
    }
}
