//! 工作窃取调度器：多 Worker 并发处理 Actor 消息。

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use crossbeam_queue::SegQueue;

use crate::runtime::RuntimeHandle;
use crate::ActorId;

/// 工作窃取调度器。
///
/// 调度模型：
/// - **全局队列**：`notify_ready` 时入队（发送方视角）；
/// - **本地队列**：Worker 处理完一批后若仍有消息，将 Actor 放回自己的
///   本地队列（亲和性，提升缓存命中）；
/// - **工作窃取**：本地 / 全局队列皆空时，随机窃取其他 Worker 本地队列
///   尾部的 Actor；
/// - **休眠与唤醒**：无任务时 Worker 通过条件变量休眠，新消息到达时唤醒。
///
/// `pending` 计数维护「就绪 + 处理中」的批次总数：每次入队 `+1`，
/// 每个批次处理完 `-1`。`pending == 0` 且收到停止信号时 Worker 退出，
/// 从而保证优雅关闭会排空所有队列。
pub(crate) struct Scheduler {
    /// Worker 数量。
    num_workers: usize,
    /// 全局就绪队列。
    global: SegQueue<ActorId>,
    /// 每个 Worker 的本地队列。
    local: Vec<Mutex<VecDeque<ActorId>>>,
    /// 待处理批次总数（入队 +1 / 处理完 -1）。
    pending: AtomicUsize,
    /// 停止信号。
    stop: AtomicBool,
    /// 休眠 / 唤醒条件变量。
    mutex: Mutex<()>,
    cond: Condvar,
}

impl Scheduler {
    /// 创建调度器（不启动线程，Worker 由 [`spawn_workers`](Self::spawn_workers) 启动）。
    pub(crate) fn new(num_workers: usize) -> Self {
        let num_workers = num_workers.max(1);
        Self {
            num_workers,
            global: SegQueue::new(),
            local: (0..num_workers)
                .map(|_| Mutex::new(VecDeque::new()))
                .collect(),
            pending: AtomicUsize::new(0),
            stop: AtomicBool::new(false),
            mutex: Mutex::new(()),
            cond: Condvar::new(),
        }
    }

    /// 启动全部 Worker 线程，返回线程句柄。
    pub(crate) fn spawn_workers(self: &Arc<Self>, runtime: Arc<RuntimeHandle>) -> Vec<JoinHandle<()>> {
        (0..self.num_workers)
            .map(|worker_id| {
                let sched = self.clone();
                let runtime = runtime.clone();
                std::thread::Builder::new()
                    .name(format!("rlyeh-worker-{worker_id}"))
                    .spawn(move || Self::worker_loop(sched, runtime, worker_id))
                    .expect("spawn worker thread")
            })
            .collect()
    }

    /// 请求停止调度（唤醒全部休眠 Worker）。
    pub(crate) fn request_stop(&self) {
        self.stop.store(true, Ordering::Release);
        self.cond.notify_all();
    }

    /// 标记 Actor 就绪（新消息到达时由发送方调用）。
    pub(crate) fn notify_ready(&self, id: ActorId) {
        self.pending.fetch_add(1, Ordering::AcqRel);
        self.global.push(id);
        self.cond.notify_one();
    }

    /// 将 Actor 放回指定 Worker 的本地队列（亲和性）。
    pub(crate) fn push_local(&self, worker_id: usize, id: ActorId) {
        self.local[worker_id].lock().unwrap().push_back(id);
    }

    /// 待处理批次计数 +1（入队时调用）。
    pub(crate) fn pending_add(&self) {
        self.pending.fetch_add(1, Ordering::AcqRel);
    }

    /// 待处理批次计数 -1（批次处理完时调用）。
    pub(crate) fn pending_sub(&self) {
        self.pending.fetch_sub(1, Ordering::AcqRel);
    }

    /// 从本地队列队首取一个 Actor。
    fn pop_local(&self, worker_id: usize) -> Option<ActorId> {
        self.local[worker_id].lock().unwrap().pop_front()
    }

    /// 工作窃取：随机挑选一个 victim，从其本地队列队尾窃取一个 Actor。
    fn steal(&self, thief_id: usize) -> Option<ActorId> {
        if self.num_workers <= 1 {
            return None;
        }
        // 随机受害者（避开自己），尝试有限次数
        for _ in 0..self.num_workers {
            let victim = (thief_id + fast_random(self.num_workers)) % self.num_workers;
            if victim == thief_id {
                continue;
            }
            if let Some(id) = self.local[victim].lock().unwrap().pop_back() {
                return Some(id);
            }
        }
        None
    }

    /// Worker 主循环。
    fn worker_loop(sched: Arc<Self>, runtime: Arc<RuntimeHandle>, worker_id: usize) {
        loop {
            // 1. 本地队列优先（亲和性）
            if let Some(id) = sched.pop_local(worker_id) {
                runtime.process_actor(id, &sched, worker_id);
                continue;
            }
            // 2. 全局队列
            if let Some(id) = sched.global.pop() {
                runtime.process_actor(id, &sched, worker_id);
                continue;
            }
            // 3. 工作窃取
            if let Some(id) = sched.steal(worker_id) {
                runtime.process_actor(id, &sched, worker_id);
                continue;
            }
            // 4. 停止检查：排空所有队列后退出
            if sched.stop.load(Ordering::Acquire) && sched.pending.load(Ordering::Acquire) == 0 {
                break;
            }
            // 5. 休眠等待新任务
            let _guard = sched.mutex.lock().unwrap();
            if sched.stop.load(Ordering::Acquire) && sched.pending.load(Ordering::Acquire) == 0 {
                break;
            }
            let _ = sched.cond.wait_timeout(_guard, std::time::Duration::from_millis(10));
        }
    }
}

/// 轻量随机数（xorshift），用于窃取时选择 victim。
fn fast_random(modulus: usize) -> usize {
    use std::sync::atomic::AtomicU64;
    static SEED: AtomicU64 = AtomicU64::new(0x9E3779B97F4A7C15);
    let mut x = SEED.fetch_add(0x9E3779B97F4A7C15, Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    SEED.store(x, Ordering::Relaxed);
    (x as usize) % modulus.max(1)
}
