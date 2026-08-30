// ===== sync 模块（B4，2026-08）：pthread 互斥锁 / 读写锁 =====
// 基于 extern FFI 绑定 pthread 家族（extern 声明于根模块 core.rl 的
// extern 集中区）；原语对象（pthread_mutex_t / pthread_rwlock_t）承载于
// malloc 缓冲，句柄（i64 指针）存于结构体字段。
// 注意：malloc 缓冲无显式释放（MVP 无析构函数），进程退出时由 OS 回收。
// trylock 系列 C 侧返回 int，声明为 `-> i32`：编译器对 ret32 extern 生成
// `declare i32` + 调用后 sext 清洗（extern_ret32），返回值可靠
// （0 = 成功 / EBUSY = 失败）。
// 分配用 `calloc` 而非 `malloc`：内建 alloc_array/alloc_bytes 已按
// `i8* @malloc(i64)` 声明 malloc，再以 i64 返回声明会触发 LLVM
// "invalid redefinition of function 'malloc'"；calloc 符号无内建冲突。
// 目录化（2026-08）：sync/module.rl = 原 sync.rl（模块规模小，保持单文件）。

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

// ===== Channel（P1，2026-08）：无界并发队列 =====
// P7c（2026-08-29）：元素类型参数化 `Channel<T>`（此前 MVP 限 i64）。
// 队列状态经 Arc<Channel<T>> 共享（Sender/Receiver 各自持有 clone；原子引用计数，可跨线程安全共享）。
// recv 空队列挂起（Condvar wait），send 后 notify_one 唤醒；
// close 后队列耗尽 recv 返回 None（try_recv 空返回 None，不阻塞）。
// 注意：queue 只增（head 单调推进，无元素移除的 MVP 简化）。
// W5（2026-08-25）：`wake_r`/`wake_w` 为 socketpair 唤醒 fd——`send`/`close`
// 向 `wake_w` 写字节，`recv_async` 的 future 经 `wake_r` 读就绪挂起（W3
// `wait_fd`/`Context.fd` 事件驱动，非阻塞线程），实现「挂起直到数据/close」。
struct Channel<T> {
    m: sync::Mutex<i64>,
    cv: sync::Condvar,
    closed: i64,
    head: i64,
    queue: Vec<T>,
    wake_r: i64,
    wake_w: i64,
    capacity: i64,   // 0 = 无界；>0 = 有界容量（满则 send 阻塞）
}

struct Sender<T> { ch: Arc<sync::Channel<T>> }
struct Receiver<T> { ch: Arc<sync::Channel<T>> }
// 元组返回类型 MVP 未实现（(1, 2) 被解析为集合），channel() 返回结构体对。
struct ChannelPair<T> { tx: sync::Sender<T>, rx: sync::Receiver<T> }

// Y4c（2026-08-30）：通道错误类型——`Result` 语义（替代 MVP `Option` 退化）。
// `recv`/`try_recv`/`send` 既有的 `Option`/`()` 便捷 API 保留以兼容既有用例；
// 错误类型经 `recv_result`/`try_recv_result`/`send_result` 暴露。
struct SendError<T> { val: T }             // send 失败：通道已关闭（无接收方）
struct RecvError { disconnected: i64 }     // recv 失败：通道已关闭且队列空
struct TryRecvError { kind: i64 }          // 0 = 空（Empty），1 = 已断开（Disconnected）

// W5（2026-08-25）：异步接收 future（`Receiver::recv_async` 返回值）。
// poll：先消费唤醒字节（避免 fd 永久就绪忙等），try_recv 非阻塞取消息——
// 有则 `Ready(Some(v))`；空且未关闭则向 `cx.fd`（`wake_r` 读）注册挂起，由事件驱动
// executor（W3 `block_on`）经 poll(2) 等 `send`/`close` 写的唤醒字节就绪再轮询。
// P7c：`Output = Option<T>`——close 且空返回 `None`（替代 MVP 哨兵 `-1`，
// 哨兵仅对 i64 有效，泛型化后改用 Option 表达「关闭且空」）。
struct RecvAsync<T> {
    ch: Arc<sync::Channel<T>>,
}

impl<T> Future for RecvAsync<T> {
    type Output = Option<T>;
    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output> {
        // 消费唤醒字节（send/close 写入），避免 fd 永久就绪导致忙等
        let _ = net::recv_some(self.ch.wake_r, 64);
        // 非阻塞取消息
        let mut r = sync::Receiver<T> { ch: self.ch.clone() };
        match r.try_recv() {
            Option::Some(v) => Poll::Ready(Option::Some(v)),
            Option::None => {
                if self.ch.closed != 0 {
                    Poll::Ready(Option::None)
                } else {
                    cx.fd = self.ch.wake_r;
                    cx.interest = 1;
                    Poll::Pending
                }
            }
        }
    }
}

// P7c：泛型通道构造——调用点用 turbofish 指定元素类型（`channel::<i64>()`）。
fn channel<T>() -> sync::ChannelPair<T> {
    // 构造调用用裸名（`sync::Mutex::new()` 路径 typecheck 不支持；
    // 裸名经 core.rl `import sync::Mutex` 别名解析为 sync::Mutex）
    // W5：创建 socketpair 唤醒 fd（`send`/`close` 写 `wake_w` 触发 `wake_r` 读就绪）
    let sp = net::socketpair_stream();
    let wake_r = net::fd_at(sp, 0);
    let wake_w = net::fd_at(sp, 1);
    // P7c：构造显式带泛型实参（`sync::Channel<T>` / `Sender<T>` / `Receiver<T>`）——
    // Arc 嵌套（`Arc<Channel<T>>`）时字段推断无法反推外层 T，须显式给出。
    let ch = Arc::new(sync::Channel<T> {
        m: Mutex::new(0),
        cv: Condvar::new(),
        closed: 0,
        head: 0,
        queue: Vec::with_capacity(8),
        wake_r: wake_r,
        wake_w: wake_w,
        capacity: 0,   // 无界
    });
    sync::ChannelPair<T> {
        tx: sync::Sender<T> { ch: ch.clone() },
        rx: sync::Receiver<T> { ch: ch },
    }
}

// Y4c（2026-08-30）：有界通道构造——`capacity > 0` 限制在途元素数，
// 满则 `send` 经 condvar 挂起阻塞（消费者腾出空间后唤醒），提供背压。
fn bounded_channel<T>(capacity: i64) -> sync::ChannelPair<T> {
    let sp = net::socketpair_stream();
    let wake_r = net::fd_at(sp, 0);
    let wake_w = net::fd_at(sp, 1);
    let ch = Arc::new(sync::Channel<T> {
        m: Mutex::new(0),
        cv: Condvar::new(),
        closed: 0,
        head: 0,
        queue: Vec::with_capacity(8),
        wake_r: wake_r,
        wake_w: wake_w,
        capacity: capacity,
    });
    sync::ChannelPair<T> {
        tx: sync::Sender<T> { ch: ch.clone() },
        rx: sync::Receiver<T> { ch: ch },
    }
}

impl<T> Sender<T> {
    // 发送（无界队列永不阻塞；唤醒一个等待中的接收者）。
    // `r#` 转义：`send` 为 actor 保留字（定义名归一化为 send，调用处 `.send(...)` 可用）
    // W5：向 `wake_w` 写唤醒字节，使 `recv_async` 挂起的 fd 读就绪（事件驱动）。
    // P7c：元素类型参数化（`val: T`）
    fn r#send(&mut self, val: T) {
        self.ch.m.lock();
        // 有界：队列满（len - head >= capacity）则挂起等待消费者腾出空间
        while self.ch.capacity > 0 && (self.ch.queue.len() - self.ch.head) >= self.ch.capacity {
            self.ch.cv.wait(self.ch.m);
        }
        self.ch.queue.push(val);
        self.ch.cv.notify_one();
        let _ = net::send_all(self.ch.wake_w, String::from("x"));
        self.ch.m.unlock();
    }
    // 尝试发送：有界队列满则返回 false；无界恒成功
    fn try_send(&mut self, val: T) -> bool {
        self.ch.m.lock();
        if self.ch.capacity > 0 && (self.ch.queue.len() - self.ch.head) >= self.ch.capacity {
            self.ch.m.unlock();
            return false;
        }
        self.ch.queue.push(val);
        self.ch.cv.notify_one();
        let _ = net::send_all(self.ch.wake_w, String::from("x"));
        self.ch.m.unlock();
        true
    }
    // send_result：Result 语义发送——通道已关闭返回 Err(SendError{val})，否则阻塞发送成功
    fn send_result(&mut self, val: T) -> Result<i64, sync::SendError<T>> {
        if self.ch.closed != 0 {
            return Result::Err(sync::SendError<T> { val: val });
        }
        self.r#send(val);
        Result::Ok(0)
    }
    // 关闭通道：广播唤醒等待者，队列耗尽后所有 Receiver recv 返回 None
    // W5：写唤醒字节，使 recv_async 挂起被唤醒（读到 close 标记）。
    fn close(&mut self) {
        self.ch.m.lock();
        self.ch.closed = 1;
        self.ch.cv.notify_all();
        let _ = net::send_all(self.ch.wake_w, String::from("x"));
        self.ch.m.unlock();
    }
    // 多 Sender 共享同一队列（Arc clone）
    fn clone(self) -> sync::Sender<T> {
        sync::Sender { ch: self.ch.clone() }
    }
}

impl<T> Receiver<T> {
    // 阻塞接收：队列空且未关闭时挂起等待；关闭且空返回 None
    fn r#recv(&mut self) -> Option<T> {
        loop {
            self.ch.m.lock();
            if self.ch.queue.len() > self.ch.head {
                let v = self.ch.queue[self.ch.head];
                self.ch.head += 1;
                self.ch.cv.notify_one();   // 唤醒阻塞中的发送者（有界队列满）
                self.ch.m.unlock();
                return Option::Some(v);
            }
            if self.ch.closed != 0 {
                self.ch.m.unlock();
                return Option::None;
            }
            self.ch.cv.wait(self.ch.m);
            self.ch.m.unlock();
        }
    }
    // 尝试接收：非空立即返回 Some(v)，空返回 None（不阻塞）
    fn try_recv(&mut self) -> Option<T> {
        self.ch.m.lock();
        if self.ch.queue.len() > self.ch.head {
            let v = self.ch.queue[self.ch.head];
            self.ch.head += 1;
            self.ch.cv.notify_one();   // 唤醒阻塞中的发送者（有界队列满）
            self.ch.m.unlock();
            return Option::Some(v);
        }
        self.ch.m.unlock();
        Option::None
    }
    // recv_result：阻塞接收 Result 语义——关闭且空返回 Err(RecvError)
    fn recv_result(&mut self) -> Result<T, sync::RecvError> {
        match self.r#recv() {
            Option::Some(v) => Result::Ok(v),
            Option::None => Result::Err(sync::RecvError { disconnected: 1 }),
        }
    }
    // try_recv_result：非阻塞 Result 语义——空 Err(kind:0)，断开 Err(kind:1)
    fn try_recv_result(&mut self) -> Result<T, sync::TryRecvError> {
        self.ch.m.lock();
        if self.ch.queue.len() > self.ch.head {
            let v = self.ch.queue[self.ch.head];
            self.ch.head += 1;
            self.ch.cv.notify_one();   // 唤醒阻塞中的发送者（有界队列满）
            self.ch.m.unlock();
            Result::Ok(v)
        } else if self.ch.closed != 0 {
            self.ch.m.unlock();
            Result::Err(sync::TryRecvError { kind: 1 })
        } else {
            self.ch.m.unlock();
            Result::Err(sync::TryRecvError { kind: 0 })
        }
    }
    // S3a/W5：异步接收——返回 `RecvAsync<T>` future，`async fn` 内经
    // `let r: sync::RecvAsync<T> = rx.recv_async(); r.await` 挂起（不阻塞线程）。
    // future 的 poll：try_recv 非阻塞取消息（有则 `Ready(Some(v))`），空且未关闭则向
    // `cx.fd`（wake_r）注册读就绪挂起；`send`/`close` 写唤醒字节触发 poll 重查。
    // P7c：`Output = Option<T>`（close 且空 → `Ready(None)`，替代 MVP 哨兵 `-1`）。
    fn recv_async(&mut self) -> sync::RecvAsync<T> {
        sync::RecvAsync<T> { ch: self.ch.clone() }
    }
    // J2 迭代器接入：for v in rx { ... }（内部 try_recv 语义，不阻塞）
    fn next(&mut self) -> Option<T> {
        self.try_recv()
    }
    // 多 Receiver 共享同一队列（Arc clone）
    fn clone(self) -> sync::Receiver<T> {
        sync::Receiver { ch: self.ch.clone() }
    }
}


