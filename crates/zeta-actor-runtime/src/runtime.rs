//! 运行时：Actor 注册表、消息处理主循环与公开入口。

use std::any::Any;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::thread::JoinHandle;

use crossbeam_queue::ArrayQueue;
use dashmap::DashMap;

use crate::actor::{ActorContext, ActorState, ActorStatus};
use crate::envelope::Envelope;
use crate::error::ActorError;
use crate::ffi::{CallbackActor, ZetaMsg};
use crate::scheduler::Scheduler;
use crate::supervisor::{RestartStrategy, Supervisor, SupervisorDecision, SupervisorStrategy};
use crate::{ActorId, ASK_TIMEOUT};

/// 默认邮箱容量。
pub const MAILBOX_CAPACITY: usize = 4096;

/// 单个 Actor 的内部句柄。
pub(crate) struct ActorHandle {
    /// Actor 私有状态。
    ///
    /// - `Option` 支持「处理期间移出」（见 [`RuntimeHandle::process_actor`]）：
    ///   Worker 处理消息时把 state 从句柄中取出、释放锁后再调用
    ///   `handle_message`，避免 `ctx.send` 重入同一 shard 写锁导致的自死锁；
    /// - `Arc<Mutex<..>>`：`ActorRef` 直接持有句柄（免去每次 DashMap 查找），
    ///   state 的可变访问需经锁；快速路径用 `try_lock`（无竞争 ~20ns）。
    pub state: Arc<Mutex<Option<Box<dyn ActorState>>>>,
    /// 无锁邮箱。
    pub mailbox: Arc<ArrayQueue<Envelope>>,
    /// 是否正在被某个 Worker 处理（CAS 保护，保证同一时刻单 Worker 访问）。
    pub running: AtomicBool,
    /// 生命周期状态（`ActorStatus` 的 `u8` 编码）。
    pub status: AtomicU8,
    /// 是否已请求停止。
    pub stop_requested: AtomicBool,
}

impl ActorHandle {
    fn new(state: Box<dyn ActorState>) -> Self {
        Self {
            state: Arc::new(Mutex::new(Some(state))),
            mailbox: Arc::new(ArrayQueue::new(MAILBOX_CAPACITY)),
            running: AtomicBool::new(false),
            status: AtomicU8::new(ActorStatus::Idle.as_u8()),
            stop_requested: AtomicBool::new(false),
        }
    }
}

/// 占位状态：仅用于 `create_ref` 在 Actor 不存在时的 fallback 句柄
/// （send / ask 仍会经注册表检查报 `ActorStopped`，此处不处理任何消息）。
struct EmptyState;

impl ActorState for EmptyState {
    fn handle_message(
        &mut self,
        _msg: Box<dyn Any + Send>,
        _ctx: &mut ActorContext,
    ) -> Result<bool, ActorError> {
        Ok(false)
    }
}

/// 运行时句柄（供 Actor 内部与 Worker 使用）。
pub struct RuntimeHandle {
    /// Actor 注册表：ID → 句柄。
    ///
    /// 值用 `Arc<ActorHandle>`：`ActorRef` 直接持有句柄引用，
    /// 使 ask / send 热路径免于每次 DashMap 分片锁查找。
    pub(crate) actors: DashMap<ActorId, Arc<ActorHandle>>,
    /// 调度器。
    pub(crate) scheduler: Arc<Scheduler>,
    /// Supervisor 管理器。
    pub(crate) supervisor: Arc<Supervisor>,
    /// 是否已进入关闭流程。
    pub(crate) stopping: AtomicBool,
    /// 反向引用（供构造 `Arc<RuntimeHandle>`）。
    weak_self: Weak<RuntimeHandle>,
}

impl RuntimeHandle {
    /// 发送消息到指定 Actor（fire-and-forget）。
    pub(crate) fn send(&self, target: ActorId, msg: Box<dyn Any + Send>) -> Result<(), ActorError> {
        if self.stopping.load(Ordering::Acquire) {
            return Err(ActorError::ShuttingDown);
        }
        let Some(handle) = self.actors.get(&target) else {
            return Err(ActorError::ActorStopped);
        };
        handle
            .mailbox
            .push(Envelope::new(None, msg))
            .map_err(|_| ActorError::MailboxFull)?;
        self.scheduler.notify_ready(target);
        Ok(())
    }

    /// 创建无监督 Actor，返回其 ID。
    pub(crate) fn spawn(&self, state: Box<dyn ActorState>) -> Result<ActorId, ActorError> {
        self.spawn_internal(state, None)
    }

    /// 创建受监督 Actor（崩溃时用 `factory` 重建状态并重启）。
    pub(crate) fn spawn_supervised<F>(
        &self,
        factory: F,
        strategy: RestartStrategy,
    ) -> Result<ActorId, ActorError>
    where
        F: Fn() -> Box<dyn ActorState> + Send + Sync + 'static,
    {
        let id = self.spawn_internal(factory(), None)?;
        let factory = Arc::new(factory);
        let strat = SupervisorStrategy {
            strategy,
            max_restarts: SupervisorStrategy::DEFAULT_MAX_RESTARTS,
            within: SupervisorStrategy::DEFAULT_WINDOW,
            children: vec![id],
            factory,
        };
        self.supervisor.register(id, strat);
        Ok(id)
    }

    /// 创建一组受监督的 Actor（共享策略，支持 AllForOne / RestartForOne）。
    pub(crate) fn spawn_group(
        &self,
        factory: &Arc<dyn Fn() -> Box<dyn ActorState> + Send + Sync>,
        strategy: RestartStrategy,
        n: usize,
    ) -> Result<Vec<ActorId>, ActorError> {
        let mut children = Vec::with_capacity(n);
        for _ in 0..n {
            let id = self.spawn_internal(factory(), None)?;
            children.push(id);
        }
        let strat = SupervisorStrategy {
            strategy,
            max_restarts: SupervisorStrategy::DEFAULT_MAX_RESTARTS,
            within: SupervisorStrategy::DEFAULT_WINDOW,
            children: children.clone(),
            factory: factory.clone(),
        };
        for &c in &children {
            self.supervisor.register(c, strat.clone());
        }
        Ok(children)
    }

    /// 创建 Actor 并注册（内部公共路径）。
    fn spawn_internal(
        &self,
        mut state: Box<dyn ActorState>,
        _parent: Option<ActorId>,
    ) -> Result<ActorId, ActorError> {
        if let Err(e) = state.init() {
            return Err(ActorError::InitFailed { reason: e.to_string() });
        }
        let id = ActorId::new();
        self.actors.insert(id, Arc::new(ActorHandle::new(state)));
        Ok(id)
    }

    /// 请求指定 Actor 停止（处理完当前消息后退出）。
    pub(crate) fn request_stop(&self, id: ActorId) {
        if let Some(handle) = self.actors.get(&id) {
            handle.stop_requested.store(true, Ordering::Release);
            handle.status.store(ActorStatus::Stopping.as_u8(), Ordering::Release);
        }
    }

    /// 创建指向指定 Actor 的引用句柄。
    pub(crate) fn create_ref(&self, id: ActorId) -> ActorRef {
        let handle = self
            .actors
            .get(&id)
            .map(|h| h.clone())
            // Actor 不存在（如已停止后被 resolve）：返回占位句柄，后续
            // send / ask 仍会经注册表检查报 `ActorStopped`。
            .unwrap_or_else(|| Arc::new(ActorHandle::new(Box::new(EmptyState))));
        ActorRef {
            id,
            handle,
            runtime: self.weak_self.upgrade().expect("runtime alive"),
        }
    }

    /// 处理一个 Actor 批次（Worker 主工作单元）。
    pub(crate) fn process_actor(&self, id: ActorId, sched: &Scheduler, worker_id: usize) {
        // ---- 第一阶段：短暂持有 DashMap 写锁，做互斥检查并提取 state ----
        // 关键：拿到 `state` 与 `mailbox` 后立即释放写锁再处理消息。
        // 否则处理期间 `ctx.send` → `actors.get` 需要同一 shard 的读锁，
        // DashMap 锁不可重入，会造成确定性死锁（偶发触发取决于 hash 分片）。
        let (mut state, mailbox) = {
            let Some(handle) = self.actors.get_mut(&id) else {
                // Actor 不存在：该批次结束
                sched.pending_sub();
                return;
            };
            // 互斥：CAS 保证同一时刻仅一个 Worker 处理该 Actor
            if handle.running.swap(true, Ordering::AcqRel) {
                // 批次对应条目已在别处处理，本批次结束
                sched.pending_sub();
                return;
            }
            if handle.stop_requested.load(Ordering::Acquire) {
                handle.running.store(false, Ordering::Release);
                drop(handle);
                sched.pending_sub();
                self.remove_actor(id);
                return;
            }
            handle.status.store(ActorStatus::Processing.as_u8(), Ordering::Release);
            // 提取 state + 共享 mailbox，随后立即释放写锁
            let state = handle.state.lock().unwrap().take().expect("actor state present");
            let mailbox = handle.mailbox.clone();
            drop(handle);
            (state, mailbox)
        };

        // ---- 第二阶段：无锁处理消息批次 ----
        let mut crash: Option<ActorError> = None;
        let mut stop_after = false;
        loop {
            let Some(env) = mailbox.pop() else { break };
            let mut ctx = ActorContext::new(
                id,
                self.weak_self.upgrade().expect("runtime alive"),
                env.reply,
            );
            match state.handle_message(env.message, &mut ctx) {
                Ok(true) => {
                    stop_after = true;
                    break;
                }
                Ok(false) => {
                    if self.stop_requested_now(id) {
                        stop_after = true;
                        break;
                    }
                }
                Err(e) => {
                    crash = Some(e);
                    break;
                }
            }
        }
        let stop_requested = self.stop_requested_now(id);

        // ---- 第三阶段：放回 state，做收尾决策 ----
        {
            let Some(handle) = self.actors.get_mut(&id) else {
                // 处理期间 Actor 被移除（如监督 Stop 路径）：state 直接丢弃
                sched.pending_sub();
                let _ = state.on_stop();
                return;
            };
            if crash.is_some() {
                // 崩溃：不放回旧 state（丢弃，由重启的新 state 替代），
                // 并保持 running=true，防止其他 Worker 在 supervisor 重启
                // 完成前取到旧 handle 处理后续消息（否则新消息会被旧 state
                // 处理，重启语义失效）。重启完成后由 handle_crash 之后的
                // 分支重置 running 并重新调度。
                *handle.state.lock().unwrap() = None;
            } else {
                handle.running.store(false, Ordering::Release);
                *handle.state.lock().unwrap() = Some(state);
                if stop_after {
                    handle.status.store(ActorStatus::Stopping.as_u8(), Ordering::Release);
                }
            }
            drop(handle);
        }
        sched.pending_sub();

        let requeue = crash.is_none() && !stop_after && !stop_requested && !mailbox.is_empty();

        if let Some(e) = crash {
            self.handle_crash(id, e);
            // 重启完成：新 handle（running=false）已就位。若邮箱仍有消息
            // （崩溃前已入队或崩溃期间新入队且被 running=true 挡住），
            // 重新调度该 Actor 处理。无 supervisor 的 Stop 分支已 remove_actor。
            if self.actors.contains_key(&id) && !mailbox.is_empty() {
                sched.pending_add();
                sched.push_local(worker_id, id);
            }
        } else if stop_after || stop_requested {
            self.remove_actor(id);
        } else if requeue {
            // 处理期间又有新消息：放回本地队列（亲和性）
            sched.pending_add();
            sched.push_local(worker_id, id);
        }
    }

    /// 查询 Actor 是否已请求停止（短暂读锁）。
    fn stop_requested_now(&self, id: ActorId) -> bool {
        self.actors
            .get(&id)
            .is_some_and(|h| h.stop_requested.load(Ordering::Acquire))
    }

    /// 处理一次崩溃：交由 Supervisor 决策并执行。
    fn handle_crash(&self, id: ActorId, error: ActorError) {
        match self.supervisor.handle_crash(id, error) {
            SupervisorDecision::Restart(ids) => {
                for victim in ids {
                    self.restart_actor(victim);
                }
            }
            SupervisorDecision::Stop | SupervisorDecision::Escalate(_) => {
                self.remove_actor(id);
            }
        }
    }

    /// 重启指定 Actor：用监督工厂重建状态，保留原 Actor ID（发送方句柄仍有效）。
    ///
    /// 复用旧邮箱：崩溃时尚未处理的消息在重启后继续处理。
    fn restart_actor(&self, id: ActorId) {
        let Some(factory) = self.supervisor.factory_for(id) else {
            self.remove_actor(id);
            return;
        };
        let mailbox = self.actors.get(&id).map(|h| h.mailbox.clone());
        let mut state = factory();
        if state.init().is_err() {
            self.remove_actor(id);
            return;
        }
        let mut handle = ActorHandle::new(state);
        if let Some(old_mailbox) = mailbox {
            handle.mailbox = old_mailbox;
        }
        // 监督策略保留（restart_actor 不 unregister supervisor）
        self.actors.insert(id, Arc::new(handle));
    }

    /// 移除 Actor：调用 `on_stop` 并清理监督关系。
    fn remove_actor(&self, id: ActorId) {
        if let Some((_, handle)) = self.actors.remove(&id) {
            handle.status.store(ActorStatus::Stopped.as_u8(), Ordering::Release);
            if let Some(mut state) = handle.state.lock().unwrap().take() {
                let _ = state.on_stop();
            }
        }
        self.supervisor.unregister(id);
    }
}

/// 外部与 Actor 交互的引用句柄。
#[derive(Clone)]
pub struct ActorRef {
    /// 目标 Actor ID。
    pub id: ActorId,
    /// 目标 Actor 的内部句柄（`Arc` 共享，热路径免去 DashMap 查找）。
    handle: Arc<ActorHandle>,
    /// 运行时句柄。
    runtime: Arc<RuntimeHandle>,
}

/// ask 快速路径尝试结果：`Handled`（已同步处理，携带回复）或 `Fallback`
/// （条件不满足，携带原消息归还调用方走慢路径）。
enum FastPathOutcome {
    /// 已由调用线程同步处理完成，携带回复。
    Handled(Box<dyn Any + Send>),
    /// 条件不满足，携带原消息回退 Worker 队列慢路径。
    Fallback(Box<dyn Any + Send>),
}

impl ActorRef {
    /// 发送消息（fire-and-forget，异步）。
    pub fn send(&self, msg: Box<dyn Any + Send>) -> Result<(), ActorError> {
        if self.runtime.stopping.load(Ordering::Acquire) {
            return Err(ActorError::ShuttingDown);
        }
        if !self.runtime.actors.contains_key(&self.id) {
            return Err(ActorError::ActorStopped);
        }
        self.handle
            .mailbox
            .push(Envelope::new(None, msg))
            .map_err(|_| ActorError::MailboxFull)?;
        self.runtime.scheduler.notify_ready(self.id);
        Ok(())
    }

    /// 发送消息并阻塞等待回复（请求-响应模式）。
    ///
    /// 接收方需在 `handle_message` 中调用 `ctx.reply(response)` 完成回复。
    /// 超时上限为 [`ASK_TIMEOUT`](crate::ASK_TIMEOUT)（默认 10 秒），
    /// 超时返回 [`ActorError::AskTimeout`]。
    ///
    /// # 快速路径（同步短路）
    ///
    /// 当目标 Actor 空闲（无 Worker 正在处理）且邮箱为空时，消息由**调用线程**
    /// 直接同步处理（无跨线程调度、无 channel 分配、无锁），大幅降低 ask 往返
    /// 开销；否则回退完整 Worker 队列路径。快速路径条件不满足（邮箱非空 /
    /// 非 Zeta 回调 Actor / 有 Worker 处理中）时语义与慢路径完全一致。
    pub fn ask_blocking<R: Any + Send>(&self, msg: Box<dyn Any + Send>) -> Result<R, ActorError> {
        if self.runtime.stopping.load(Ordering::Acquire) {
            return Err(ActorError::ShuttingDown);
        }
        if !self.runtime.actors.contains_key(&self.id) {
            return Err(ActorError::ActorStopped);
        }
        // ---- 快速路径：空闲 + 空邮箱 → 调用线程直接处理 ----
        let msg = match self.try_fast_ask(msg) {
            FastPathOutcome::Handled(reply) => {
                return reply
                    .downcast::<R>()
                    .map(|b| *b)
                    .map_err(|_| ActorError::WrongReplyType);
            }
            FastPathOutcome::Fallback(msg) => msg,
        };
        // ---- 慢路径：完整 Worker 队列（原有语义） ----
        let (tx, rx) = crossbeam_channel::bounded::<Box<dyn Any + Send>>(1);
        let env = Envelope::with_reply(None, msg, tx);
        self.handle
            .mailbox
            .push(env)
            .map_err(|_| ActorError::MailboxFull)?;
        self.runtime.scheduler.notify_ready(self.id);
        let resp = rx
            .recv_timeout(ASK_TIMEOUT)
            .map_err(|_| ActorError::AskTimeout)?;
        resp.downcast::<R>()
            .map(|b| *b)
            .map_err(|_| ActorError::WrongReplyType)
    }

    /// 发送消息并等待回复（async 形态）。
    ///
    /// MVP 无内建 async executor：内部等价于 [`ask_blocking`](Self::ask_blocking)，
    /// 阻塞调用线程直到收到回复。
    pub async fn ask<R: Any + Send>(&self, msg: Box<dyn Any + Send>) -> Result<R, ActorError> {
        self.ask_blocking(msg)
    }

    /// 快速路径尝试：空闲 + 空邮箱时由调用线程直接同步处理消息。
    ///
    /// 返回 `FastPathOutcome::Handled(reply)` 表示已同步处理完成；`Fallback`
    /// 表示条件不满足（原消息原样归还），调用方应回退 Worker 队列慢路径
    /// （语义与慢路径完全一致）。
    ///
    /// 正确性论证：
    /// - **互斥**：`running` CAS 抢占处理权，与 Worker 的
    ///   [`process_actor`](RuntimeHandle::process_actor) 同一互斥机制；
    /// - **FIFO**：邮箱非空（先入队消息存在）时回退慢路径，不插队；
    /// - **并发安全**：处理期间入队（含 self-ask）由收尾的"邮箱非空 →
    ///   `notify_ready`"兜底：被唤醒的 Worker 会再次尝试抢占并处理，
    ///   `running` 已释放故能成功；
    /// - **崩溃语义**：返回 `-1` 时回复 0、丢弃旧状态、保持 `running=true`
    ///   并交由 supervisor 决策，与 [`process_actor`](RuntimeHandle::process_actor)
    ///   崩溃分支一致。
    ///
    /// 注意：本方法在 `running` CAS 成功后**不得提前消费 `msg`**（所有
    /// 回退路径需把原消息归还给慢路径），故 `ZetaMsg` 的 downcast 延迟到
    /// 所有前置条件确认之后。
    fn try_fast_ask(&self, msg: Box<dyn Any + Send>) -> FastPathOutcome {
        let handle = &self.handle;
        // CAS 抢占处理权：已有 Worker 处理中则回退慢路径。
        if handle.running.swap(true, Ordering::AcqRel) {
            return FastPathOutcome::Fallback(msg);
        }
        // FIFO 约束：邮箱已有先入队消息时不可插队。
        // 抢锁窗口内可能已有 send 触发 notify，收尾再补一次 notify 确保排队
        // 消息被调度（若 Worker 恰好处理完则该调用为无害空转）。
        if !handle.mailbox.is_empty() {
            handle.running.store(false, Ordering::Release);
            self.runtime.scheduler.notify_ready(self.id);
            return FastPathOutcome::Fallback(msg);
        }
        // state 访问经 Arc<Mutex<..>>；快速路径用 try_lock（无竞争 ~20ns），
        // 抢锁失败（罕见：Worker 恰好移出 state）则回退慢路径。
        let mut state_guard = match handle.state.try_lock() {
            Ok(g) => g,
            Err(_) => {
                handle.running.store(false, Ordering::Release);
                self.runtime.scheduler.notify_ready(self.id);
                return FastPathOutcome::Fallback(msg);
            }
        };
        // 仅对 Zeta 生成的 CallbackActor 启用快速路径（自定义 ActorState 一律
        // 回退慢路径）。利用 supertrait upcasting：`&mut dyn ActorState` →
        // `&mut dyn Any`（rustc >= 1.86）。
        let mut state_opt = state_guard.as_mut(); // &mut Option<Box<dyn ActorState>>
        let cb = match state_opt.as_mut().and_then(|s| {
            let state: &mut dyn ActorState = s.as_mut();
            let any: &mut dyn Any = state;
            any.downcast_mut::<CallbackActor>()
        }) {
            Some(cb) => cb,
            None => {
                handle.running.store(false, Ordering::Release);
                self.runtime.scheduler.notify_ready(self.id);
                return FastPathOutcome::Fallback(msg);
            }
        };
        // 前置条件全部满足，此时才消费消息。
        let zm = match msg.downcast::<ZetaMsg>() {
            Ok(m) => *m,
            Err(msg) => {
                handle.running.store(false, Ordering::Release);
                self.runtime.scheduler.notify_ready(self.id);
                return FastPathOutcome::Fallback(msg);
            }
        };
        let handler = cb.handler;
        let state_val = cb.state;
        // 同线程直接调用 Zeta handle（状态槽内存按 u64 传递，与 Worker 一致）。
        #[allow(unsafe_code)] // FFI 调用 Zeta extern "C" handler，必要且已校验符号存在
        let ret = unsafe { handler(state_val, zm.kind, zm.a, zm.b, zm.c) };
        if ret == u64::MAX {
            // 崩溃：回复 0、丢弃旧 state、保持 running=true（防止重启完成前
            // 其他 Worker 取旧句柄处理后续消息），交由 supervisor 决策。
            *state_guard = None;
            self.runtime.handle_crash(
                self.id,
                ActorError::Panic {
                    reason: "zeta handle 返回 -1（处理出错）".into(),
                },
            );
            // 重启成功后（新句柄 running=false）遗留消息需要重新调度。
            if self.runtime.actors.contains_key(&self.id) && !handle.mailbox.is_empty() {
                self.runtime.scheduler.notify_ready(self.id);
            }
            return FastPathOutcome::Handled(Box::new(0u64));
        }
        handle.running.store(false, Ordering::Release);
        // 处理期间新入队消息（含 self-ask / 并发 send）→ 重新调度。
        if !handle.mailbox.is_empty() {
            self.runtime.scheduler.notify_ready(self.id);
        }
        FastPathOutcome::Handled(Box::new(ret))
    }

    /// 请求目标 Actor 停止。
    pub fn stop(&self) {
        self.runtime.request_stop(self.id);
    }

    /// 目标 Actor 的 ID。
    pub fn id(&self) -> ActorId {
        self.id
    }

    /// 查询目标 Actor 的生命周期状态。
    pub fn status(&self) -> ActorStatus {
        self.runtime
            .actors
            .get(&self.id)
            .map(|h| ActorStatus::from_u8(h.status.load(Ordering::Acquire)))
            .unwrap_or(ActorStatus::Stopped)
    }
}

impl std::fmt::Debug for ActorRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActorRef").field("id", &self.id).finish()
    }
}

/// 运行时构建器。
pub struct RuntimeBuilder {
    num_workers: usize,
    max_actors: usize,
}

impl RuntimeBuilder {
    /// 默认配置：Worker 数 = 逻辑 CPU 数。
    pub fn new() -> Self {
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        Self {
            num_workers: cores,
            max_actors: 10_000,
        }
    }

    /// 设置 Worker 线程数。
    pub fn with_workers(mut self, n: usize) -> Self {
        self.num_workers = n.max(1);
        self
    }

    /// 设置最大 Actor 数（MVP 保留字段，用于未来配额控制）。
    pub fn with_max_actors(mut self, n: usize) -> Self {
        self.max_actors = n;
        self
    }

    /// 构建并启动运行时。
    pub fn build(self) -> Runtime {
        let supervisor = Arc::new(Supervisor::new());
        let scheduler = Arc::new(Scheduler::new(self.num_workers));
        let handle = Arc::new_cyclic(|weak| RuntimeHandle {
            actors: DashMap::with_capacity(256),
            scheduler: scheduler.clone(),
            supervisor,
            stopping: AtomicBool::new(false),
            weak_self: weak.clone(),
        });
        let workers = scheduler.spawn_workers(handle.clone());
        Runtime {
            handle,
            scheduler,
            workers,
            timer_threads: Mutex::new(Vec::new()),
            _max_actors: self.max_actors,
        }
    }
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// 运行时实例。
pub struct Runtime {
    handle: Arc<RuntimeHandle>,
    scheduler: Arc<Scheduler>,
    workers: Vec<JoinHandle<()>>,
    timer_threads: Mutex<Vec<JoinHandle<()>>>,
    _max_actors: usize,
}

impl Runtime {
    /// 创建无监督 Actor。
    pub fn spawn<A: ActorState>(&self, state: A) -> ActorRef {
        self.spawn_boxed(Box::new(state))
    }

    /// 以 `Box<dyn ActorState>` 创建无监督 Actor。
    pub fn spawn_boxed(&self, state: Box<dyn ActorState>) -> ActorRef {
        match self.handle.spawn(state) {
            Ok(id) => self.handle.create_ref(id),
            Err(e) => panic!("spawn failed: {e}"),
        }
    }

    /// 创建受监督的 Actor（崩溃时按策略重启）。
    ///
    /// `factory` 用于崩溃后重建状态对象。
    pub fn spawn_supervised<F>(
        &self,
        factory: F,
        strategy: RestartStrategy,
    ) -> ActorRef
    where
        F: Fn() -> Box<dyn ActorState> + Send + Sync + 'static,
    {
        let id = self
            .handle
            .spawn_supervised(factory, strategy)
            .expect("spawn supervised actor");
        self.handle.create_ref(id)
    }

    /// 创建一组受监督的 Actor（共享策略与工厂）。
    ///
    /// `AllForOne` / `RestartForOne` 作用于该组成员。
    pub fn spawn_group<F>(
        &self,
        factory: F,
        strategy: RestartStrategy,
        n: usize,
    ) -> Vec<ActorRef>
    where
        F: Fn() -> Box<dyn ActorState> + Send + Sync + 'static,
    {
        let factory: Arc<dyn Fn() -> Box<dyn ActorState> + Send + Sync> = Arc::new(factory);
        let ids = self
            .handle
            .spawn_group(&factory, strategy, n)
            .expect("spawn actor group");
        ids.into_iter()
            .map(|id| self.handle.create_ref(id))
            .collect()
    }

    /// 创建周期性 Timer Actor：每隔 `interval` 触发一次 `callback`。
    ///
    /// 回调在 Timer Actor 的消息处理线程中执行。
    pub fn spawn_timer(
        &self,
        interval: std::time::Duration,
        callback: Box<dyn Fn() + Send + Sync>,
    ) -> ActorRef {
        let timer = crate::builtin::Timer { interval, callback };
        let actor = self.spawn_boxed(Box::new(timer));
        let handle = self.handle.clone();
        let id = actor.id;
        let thread = std::thread::Builder::new()
            .name("zeta-timer".to_string())
            .spawn(move || {
                while !handle.stopping.load(Ordering::Acquire) {
                    std::thread::sleep(interval);
                    if handle.stopping.load(Ordering::Acquire) {
                        break;
                    }
                    let _ = handle.send(id, Box::new(crate::builtin::TimerTick));
                }
            })
            .expect("spawn timer thread");
        self.timer_threads.lock().unwrap().push(thread);
        actor
    }

    /// 由 Actor ID 解析出引用句柄（例如父 Actor 通过消息传递子 Actor ID 后，
    /// 外部需要重建句柄进行交互）。
    pub fn resolve(&self, id: ActorId) -> ActorRef {
        self.handle.create_ref(id)
    }

    /// 运行直到 [`shutdown`](Self::shutdown) 被调用（阻塞当前线程）。
    ///
    /// MVP 实现：阻塞等待，直到 `shutdown` 触发停止。
    pub fn run(&self) {
        while !self.handle.stopping.load(Ordering::Acquire) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// 优雅关闭：排空所有队列 → 停止 Worker → 调用所有 Actor 的 `on_stop`。
    pub fn shutdown(self) {
        // 1. 设置停止信号并唤醒全部 Worker
        self.handle.stopping.store(true, Ordering::Release);
        self.scheduler.request_stop();
        // 2. 等待 Worker 排空队列后退出
        for w in self.workers {
            let _ = w.join();
        }
        // 2.1 停止 Timer 线程
        for t in std::mem::take(&mut *self.timer_threads.lock().unwrap()) {
            let _ = t.join();
        }
        // 3. 剩余 Actor 清理（调用 on_stop）
        for entry in self.handle.actors.iter_mut() {
            entry
                .status
                .store(ActorStatus::Stopped.as_u8(), Ordering::Release);
            if let Some(mut state) = entry.state.lock().unwrap().take() {
                let _ = state.on_stop();
            }
        }
        self.handle.actors.clear();
        self.handle.supervisor.clear();
    }
}

impl std::fmt::Debug for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime")
            .field("workers", &self.workers.len())
            .field("actors", &self.handle.actors.len())
            .finish()
    }
}
