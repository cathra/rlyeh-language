# P006: Actor 运行时

> **模块路径**：`crates/rlyeh-actor-runtime/`  
> **预估工期**：5-7 天  
> **前置依赖**：P004（区域系统，用于 Actor 状态分配）  
> **输出**：可工作的 Actor 运行时，支持消息传递、调度、Supervisor

---

## 任务描述

实现 Rlyeh 的 Actor 并发模型运行时：
1. **Actor 创建与状态管理**
2. **消息队列与分发**
3. **工作窃取调度器**
4. **Supervisor 与崩溃恢复**

---

## 核心设计

### Actor 运行时架构

```
┌─────────────────────────────────────────┐
│           Actor Runtime                  │
│                                         │
│  ┌─────────────┐    ┌─────────────┐    │
│  │ Scheduler   │    │ Supervisor  │    │
│  │ (WorkSteal) │    │ Manager     │    │
│  └──────┬──────┘    └──────┬──────┘    │
│         │                   │            │
│  ┌──────▼──────────────────▼──────┐    │
│  │      Actor Registry             │    │
│  │  id → ActorHandle              │    │
│  └──────┬──────────────────┬──────┘    │
│         │                   │            │
│  ┌──────▼──────┐    ┌─────▼──────┐    │
│  │ Actor Mailbox│    │ Actor State │    │
│  │ (MPMC Queue)│    │ (Heap Alloc)│    │
│  └─────────────┘    └────────────┘    │
│                                         │
│  ┌─────────────────────────────────┐    │
│  │   Worker Threads (N = CPU cores)│    │
│  └─────────────────────────────────┘    │
└─────────────────────────────────────────┘
```

---

## 代码框架

```rust
// crates/rlyeh-actor-runtime/Cargo.toml
[package]
name = "rlyeh-actor-runtime"
version = "0.1.0"
edition = "2021"

[dependencies]
crossbeam-queue = "0.3"
crossbeam-channel = "0.5"
parking_lot = "0"
dashmap = "5"
rayon = "1"
once_cell = "1"
anyhow = "1"
thiserror = "1"
uuid = { version = "1", features = ["v4"] }
```

```rust
// crates/rlyeh-actor-runtime/src/lib.rs

#![warn(missing_docs)]
#![warn(unsafe_code)]

use std::any::Any;
use std::sync::Arc;
use std::collections::HashMap;
use dashmap::DashMap;
use thiserror::Error;

// ===== Actor ID =====

/// 唯一标识一个 Actor 实例
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActorId(uuid::Uuid);

impl ActorId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

// ===== Actor 状态 =====

/// Actor 的私有状态
/// 每个 Actor 拥有独立的状态对象
pub trait ActorState: Any + Send + 'static {
    /// Actor 初始化
    fn init(&mut self) -> Result<(), ActorError>;
    
    /// 处理一条消息
    /// 返回是否需要停止
    fn handle_message(
        &mut self,
        msg: Box<dyn Any + Send>,
        ctx: &mut ActorContext,
    ) -> Result<bool, ActorError>;
    
    /// Actor 即将停止
    fn on_stop(&mut self) -> Result<(), ActorError> {
        Ok(())
    }
}

// ===== Actor 上下文 =====

/// 在消息处理期间可用的上下文
pub struct ActorContext {
    pub self_id: ActorId,
    pub runtime: Arc<RuntimeHandle>,
}

impl ActorContext {
    /// 向另一个 Actor 发送消息
    pub fn send(&self, target: ActorId, msg: Box<dyn Any + Send>) {
        self.runtime.send(target, msg);
    }
    
    /// 创建子 Actor
    pub fn spawn(&self, state: Box<dyn ActorState>) -> ActorId {
        self.runtime.spawn(state)
    }
    
    /// 停止自身
    pub fn stop(&self) {
        // 设置停止标志
    }
}

// ===== 消息信封 =====

struct Envelope {
    sender: Option<ActorId>,
    message: Box<dyn Any + Send>,
}

// ===== Actor 句柄 =====

/// 外部用来与 Actor 交互的句柄
pub struct ActorRef {
    id: ActorId,
    mailbox: Arc<crossbeam_queue::ArrayQueue<Envelope>>,
    runtime: Arc<RuntimeHandle>,
}

impl ActorRef {
    /// 发送消息（异步，不等待回复）
    pub fn send(&self, msg: Box<dyn Any + Send>) {
        let envelope = Envelope {
            sender: None,
            message: msg,
        };
        // 非阻塞发送
        let _ = self.mailbox.push(envelope);
        // 通知调度器
        self.runtime.notify_ready(self.id);
    }
    
    /// 发送消息并等待回复
    pub async fn ask<R: Any + Send>(&self, msg: Box<dyn Any + Send>) -> Result<R, ActorError> {
        // 创建一次性 channel
        let (tx, rx) = crossbeam_channel::bounded(1);
        
        let envelope = Envelope {
            sender: None,
            message: Box::new((msg, tx)),
        };
        self.mailbox.push(envelope).map_err(|_| ActorError::MailboxFull)?;
        self.runtime.notify_ready(self.id);
        
        // 等待回复
        rx.recv().map_err(|_| ActorError::ActorStopped)
            .and_then(|resp| {
                resp.downcast::<R>()
                    .map(|b| *b)
                    .map_err(|_| ActorError::WrongReplyType)
            })
    }
    
    pub fn id(&self) -> ActorId {
        self.id
    }
}

// ===== 运行时 =====

/// 运行时句柄（供 Actor 内部使用）
pub struct RuntimeHandle {
    actors: DashMap<ActorId, ActorHandle>,
    scheduler: Arc<Scheduler>,
    supervisor: Arc<Supervisor>,
}

struct ActorHandle {
    state: Box<dyn ActorState>,
    mailbox: Arc<crossbeam_queue::ArrayQueue<Envelope>>,
    context: ActorContext,
    status: ActorStatus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ActorStatus {
    Idle,
    Processing,
    Stopping,
    Stopped,
    Crashed,
}

impl RuntimeHandle {
    fn send(&self, target: ActorId, msg: Box<dyn Any + Send>) {
        if let Some(handle) = self.actors.get(&target) {
            let envelope = Envelope {
                sender: None,
                message: msg,
            };
            let _ = handle.mailbox.push(envelope);
            self.scheduler.notify_ready(target);
        }
    }
    
    fn spawn(&self, state: Box<dyn ActorState>) -> ActorId {
        let id = ActorId::new();
        let mailbox = Arc::new(
            crossbeam_queue::ArrayQueue::new(256)
        );
        
        let handle = ActorHandle {
            context: ActorContext {
                self_id: id,
                runtime: Arc::new(self.clone()),
            },
            state,
            mailbox: mailbox.clone(),
            status: ActorStatus::Idle,
        };
        
        self.actors.insert(id, handle);
        self.scheduler.register(id, mailbox);
        id
    }
    
    fn notify_ready(&self, id: ActorId) {
        self.scheduler.notify_ready(id);
    }
}

// ===== 调度器 =====

struct Scheduler {
    /// 全局就绪队列（工作窃取）
    ready_queue: crossbeam_queue::SegQueue<ActorId>,
    
    /// 每个 Worker 的本地队列
    worker_queues: Vec<crossbeam_queue::SegQueue<ActorId>>,
    
    /// Worker 线程池
    workers: Vec<std::thread::JoinHandle<()>>,
    
    /// 运行时句柄
    runtime: Arc<RuntimeHandle>,
}

impl Scheduler {
    pub fn new(num_workers: usize, runtime: Arc<RuntimeHandle>) -> Self {
        todo!("创建 N 个 Worker 线程");
        todo!("每个 Worker 有本地队列");
        todo!("全局队列用于工作窃取");
    }
    
    pub fn register(&self, id: ActorId, mailbox: Arc<crossbeam_queue::ArrayQueue<Envelope>>) {
        todo!("注册 Actor 到调度器");
    }
    
    pub fn notify_ready(&self, id: ActorId) {
        // 放入全局就绪队列
        self.ready_queue.push(id);
        // 唤醒一个 Worker
    }
    
    /// Worker 主循环
    fn worker_loop(
        worker_id: usize,
        local_queue: crossbeam_queue::SegQueue<ActorId>,
        global_queue: Arc<crossbeam_queue::SegQueue<ActorId>>,
        runtime: Arc<RuntimeHandle>,
    ) {
        loop {
            // 1. 尝试从本地队列取
            // 2. 尝试从全局队列取
            // 3. 尝试从其他 Worker 窃取
            // 4. 如果都空，等待通知
            
            todo!("完整的 Worker 调度循环");
        }
    }
    
    /// 工作窃取：从其他 Worker 偷一半任务
    fn steal_work(&self, thief_id: usize) -> Option<ActorId> {
        todo!("随机选一个 victim");
        todo!("从 victim 的队列偷一半");
    }
}

// ===== Supervisor =====

struct Supervisor {
    strategies: DashMap<ActorId, SupervisorStrategy>,
    restart_counts: DashMap<ActorId, RestartCounter>,
}

struct SupervisorStrategy {
    strategy: RestartStrategy,
    max_restarts: usize,
    within: std::time::Duration,
    children: Vec<ActorId>,
}

#[derive(Debug, Clone)]
enum RestartStrategy {
    /// 只重启崩溃的 Actor
    OneForOne,
    /// 一个崩溃，全部重启
    AllForOne,
    /// 重启崩溃者及其后续声明的
    RestOnForOne,
}

struct RestartCounter {
    count: usize,
    window_start: std::time::Instant,
}

impl Supervisor {
    pub fn new() -> Self {
        Self {
            strategies: DashMap::new(),
            restart_counts: DashMap::new(),
        }
    }
    
    /// 处理 Actor 崩溃
    pub fn handle_crash(
        &self,
        actor_id: ActorId,
        error: ActorError,
    ) -> SupervisorDecision {
        todo!("查找该 Actor 的 Supervisor");
        todo!("检查重启策略");
        todo!("检查重启频率限制");
        todo!("决定：重启 / 停止 / 升级错误");
    }
    
    /// 注册 Supervisor
    pub fn register(
        &self,
        supervisor_id: ActorId,
        strategy: SupervisorStrategy,
    ) {
        self.strategies.insert(supervisor_id, strategy);
    }
}

enum SupervisorDecision {
    Restart,
    Stop,
    Escalate(ActorError),
}

// ===== Actor 错误 =====

#[derive(Debug, Error)]
pub enum ActorError {
    #[error("actor mailbox is full")]
    MailboxFull,
    
    #[error("actor has stopped")]
    ActorStopped,
    
    #[error("wrong reply type")]
    WrongReplyType,
    
    #[error("actor panicked: {reason}")]
    Panic { reason: String },
    
    #[error("supervisor restart limit exceeded for actor {actor_id:?}")]
    RestartLimitExceeded { actor_id: ActorId },
    
    #[error("actor initialization failed: {reason}")]
    InitFailed { reason: String },
}

// ===== 运行时构建器 =====

/// 创建并配置运行时
pub struct RuntimeBuilder {
    num_workers: usize,
    max_actors: usize,
}

impl RuntimeBuilder {
    pub fn new() -> Self {
        Self {
            num_workers: num_cpus::get(),
            max_actors: 10000,
        }
    }
    
    pub fn with_workers(mut self, n: usize) -> Self {
        self.num_workers = n;
        self
    }
    
    pub fn with_max_actors(mut self, n: usize) -> Self {
        self.max_actors = n;
        self
    }
    
    pub fn build(self) -> Runtime {
        todo!("创建完整的运行时");
    }
}

/// 运行时实例
pub struct Runtime {
    handle: Arc<RuntimeHandle>,
    scheduler: Arc<Scheduler>,
    supervisor: Arc<Supervisor>,
    workers: Vec<std::thread::JoinHandle<()>>,
}

impl Runtime {
    /// 创建根 Actor（返回 ActorRef）
    pub fn spawn<A: ActorState>(&self, state: A) -> ActorRef {
        let id = self.handle.spawn(Box::new(state));
        self.create_ref(id)
    }
    
    /// 运行直到所有 Actor 停止
    pub fn run(&self) {
        // 等待所有 Worker 线程
    }
    
    /// 优雅关闭
    pub fn shutdown(self) {
        // 发送停止信号给所有 Actor
        // 等待处理完队列中的消息
        // 调用 on_stop
        // 停止 Worker 线程
    }
    
    fn create_ref(&self, id: ActorId) -> ActorRef {
        // 从 handle 中获取 mailbox
        todo!()
    }
}

// ===== 内置 Actor 工具 =====

/// 简单的消息路由 Actor
pub struct Router {
    routes: HashMap<String, ActorId>,
}

impl ActorState for Router {
    fn handle_message(
        &mut self,
        msg: Box<dyn Any + Send>,
        ctx: &mut ActorContext,
    ) -> Result<bool, ActorError> {
        // 根据消息类型路由
        todo!()
    }
}

/// 周期性定时器 Actor
pub struct Timer {
    interval: std::time::Duration,
    callback: Box<dyn Fn() + Send>,
}

impl ActorState for Timer {
    fn handle_message(
        &mut self,
        _msg: Box<dyn Any + Send>,
        _ctx: &mut ActorContext,
    ) -> Result<bool, ActorError> {
        // 定时触发 callback
        todo!()
    }
}
```

---

## 必须实现的功能清单

### 1. 核心运行时
- [ ] Runtime 创建与销毁
- [ ] Actor 注册与查找
- [ ] 消息发送（fire-and-forget）
- [ ] 请求-响应（ask 模式）
- [ ] Actor 优雅停止

### 2. 调度器
- [ ] 工作窃取算法
- [ ] 本地队列 + 全局队列
- [ ] Worker 唤醒机制
- [ ] 空闲 Worker 休眠

### 3. 消息系统
- [ ] 无锁邮箱（crossbeam ArrayQueue）
- [ ] 回压机制（mailbox 满时）
- [ ] 消息优先级（可选）
- [ ] 批量消息处理

### 4. Supervisor
- [ ] OneForOne 策略
- [ ] AllForOne 策略
- [ ] RestForOne 策略
- [ ] 重启频率限制
- [ ] 崩溃传播

### 5. 错误处理
- [ ] Actor panic 捕获
- [ ] 错误升级链
- [ ] 监控接口

---

## 测试用例

```rust
// crates/rlyeh-actor-runtime/tests/actor_test.rs

use rlyeh_actor_runtime::*;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

/// 简单的计数器 Actor
struct Counter {
    value: i32,
}

impl ActorState for Counter {
    fn handle_message(
        &mut self,
        msg: Box<dyn Any + Send>,
        _ctx: &mut ActorContext,
    ) -> Result<bool, ActorError> {
        if let Some(&delta) = msg.downcast_ref::<i32>() {
            self.value += delta;
        }
        Ok(false) // 不停止
    }
}

#[test]
fn test_basic_send() {
    let runtime = RuntimeBuilder::new().with_workers(2).build();
    
    let counter = Counter { value: 0 };
    let ref = runtime.spawn(counter);
    
    // 发送消息
    for i in 0..100 {
        ref.send(Box::new(i));
    }
    
    // 等待处理
    std::thread::sleep(std::time::Duration::from_millis(100));
    
    runtime.shutdown();
    // 验证 counter.value == 4950
}

#[test]
fn test_ask_pattern() {
    struct Echo;
    impl ActorState for Echo {
        fn handle_message(
            &mut self,
            msg: Box<dyn Any + Send>,
            _ctx: &mut ActorContext,
        ) -> Result<bool, ActorError> {
            // Echo 回传消息
            Ok(false)
        }
    }
    
    let runtime = RuntimeBuilder::new().build();
    let echo = runtime.spawn(Echo);
    
    // ask 模式需要特殊消息包装
    // 这里简化处理
    runtime.shutdown();
}

#[test]
fn test_supervisor_restart() {
    static CRASH_COUNT: AtomicI32 = AtomicI32::new(0);
    
    struct FlakyActor;
    impl ActorState for FlakyActor {
        fn handle_message(
            &mut self,
            _msg: Box<dyn Any + Send>,
            _ctx: &mut ActorContext,
        ) -> Result<bool, ActorError> {
            let count = CRASH_COUNT.fetch_add(1, Ordering::SeqCst);
            if count < 3 {
                Err(ActorError::Panic { reason: "boom".to_string() })
            } else {
                Ok(false) // 第 4 次成功
            }
        }
    }
    
    let runtime = RuntimeBuilder::new().build();
    
    // 注册 Supervisor
    let flaky = runtime.spawn(FlakyActor);
    
    // 发送消息触发崩溃和重启
    for _ in 0..5 {
        flaky.send(Box::new(()));
    }
    
    std::thread::sleep(std::time::Duration::from_millis(200));
    
    // 验证重启了 3 次
    assert_eq!(CRASH_COUNT.load(Ordering::SeqCst), 4);
    
    runtime.shutdown();
}

#[test]
fn test_message_ordering() {
    /// 验证同一发送者的消息顺序
    struct OrderTracker {
        last_seen: i32,
        out_of_order: bool,
    }
    
    impl ActorState for OrderTracker {
        fn handle_message(
            &mut self,
            msg: Box<dyn Any + Send>,
            _ctx: &mut ActorContext,
        ) -> Result<bool, ActorError> {
            if let Some(&n) = msg.downcast_ref::<i32>() {
                if n != self.last_seen + 1 {
                    self.out_of_order = true;
                }
                self.last_seen = n;
            }
            Ok(false)
        }
    }
    
    let runtime = RuntimeBuilder::new().build();
    let tracker = runtime.spawn(OrderTracker { last_seen: 0, out_of_order: false });
    
    // 同一发送者顺序发送
    for i in 1..1000 {
        tracker.send(Box::new(i));
    }
    
    std::thread::sleep(std::time::Duration::from_millis(100));
    runtime.shutdown();
    
    // 验证顺序正确
}

#[test]
fn test_actor_isolation() {
    /// 一个 Actor 崩溃不应影响其他 Actor
    struct PoisonPill;
    
    let runtime = RuntimeBuilder::new().with_workers(4).build();
    
    // 创建多个 Actor
    let actors: Vec<_> = (0..10)
        .map(|_| runtime.spawn(Counter { value: 0 }))
        .collect();
    
    // 让一个崩溃
    // 验证其他继续工作
    
    runtime.shutdown();
}

#[test]
fn test_graceful_shutdown() {
    struct LongRunning;
    impl ActorState for LongRunning {
        fn handle_message(
            &mut self,
            _msg: Box<dyn Any + Send>,
            _ctx: &mut ActorContext,
        ) -> Result<bool, ActorError> {
            std::thread::sleep(std::time::Duration::from_millis(10));
            Ok(false)
        }
    }
    
    let runtime = RuntimeBuilder::new().build();
    let actor = runtime.spawn(LongRunning);
    
    // 发送一些消息
    for _ in 0..100 {
        actor.send(Box::new(()));
    }
    
    // 优雅关闭应该等待消息处理完
    let start = std::time::Instant::now();
    runtime.shutdown();
    let elapsed = start.elapsed();
    
    // 应该至少等待了消息处理
    assert!(elapsed >= std::time::Duration::from_millis(50));
}

#[test]
fn test_spawn_chain() {
    /// Actor 在消息处理中创建子 Actor
    struct Parent {
        child_ref: Option<ActorRef>,
    }
    
    impl ActorState for Parent {
        fn handle_message(
            &mut self,
            msg: Box<dyn Any + Send>,
            ctx: &mut ActorContext,
        ) -> Result<bool, ActorError> {
            if let Some(&n) = msg.downcast_ref::<i32>() {
                if self.child_ref.is_none() {
                    let child = Counter { value: *n };
                    let child_ref = ctx.spawn(Box::new(child));
                    self.child_ref = Some(child_ref);
                }
            }
            Ok(false)
        }
    }
    
    let runtime = RuntimeBuilder::new().build();
    let parent = runtime.spawn(Parent { child_ref: None });
    
    parent.send(Box::new(42i32));
    
    std::thread::sleep(std::time::Duration::from_millis(100));
    runtime.shutdown();
}

#[test]
fn test_high_throughput() {
    /// 测试吞吐量
    struct Sink {
        count: usize,
    }
    
    impl ActorState for Sink {
        fn handle_message(
            &mut self,
            _msg: Box<dyn Any + Send>,
            _ctx: &mut ActorContext,
        ) -> Result<bool, ActorError> {
            self.count += 1;
            Ok(false)
        }
    }
    
    let runtime = RuntimeBuilder::new().with_workers(num_cpus::get()).build();
    let sink = runtime.spawn(Sink { count: 0 });
    
    let start = std::time::Instant::now();
    let total = 1_000_000;
    
    for i in 0..total {
        sink.send(Box::new(i));
    }
    
    // 等待处理完成
    std::thread::sleep(std::time::Duration::from_secs(2));
    let elapsed = start.elapsed();
    
    runtime.shutdown();
    
    let throughput = total as f64 / elapsed.as_secs_f64();
    println!("Throughput: {:.0} msg/s", throughput);
    
    // 目标：> 1M msg/s
}
```

---

## 性能基准

```rust
// crates/rlyeh-actor-runtime/benches/actor_bench.rs
use criterion::{black_box, Criterion};
use rlyeh_actor_runtime::*;

fn bench_send_only(c: &mut Criterion) {
    c.bench_function("send_100k", |b| {
        let runtime = RuntimeBuilder::new().with_workers(2).build();
        let sink = runtime.spawn(CountingSink::new());
        
        b.iter(|| {
            for i in 0..100000 {
                sink.send(black_box(Box::new(i)));
            }
            // 等待完成
            std::thread::sleep(std::time::Duration::from_millis(50));
        });
        
        runtime.shutdown();
    });
}

// 目标：
// - 消息发送延迟 < 50ns（fire-and-forget）
// - 吞吐量 > 5M msg/s（4 核）
// - Actor 创建 < 10μs
```

---

## 验收标准

| 标准 | 要求 |
|------|------|
| 所有测试通过 | 100% |
| 零警告 | `clippy -- -D warnings` |
| 吞吐量 | > 1M msg/s（单线程） |
| 延迟 | 发送 < 100ns |
| 崩溃隔离 | 一个 Actor 崩溃不影响其他 |
| 优雅关闭 | 处理完队列后退出 |
| 工作窃取 | 负载均衡 < 10% 偏差 |

---

## 交付文件

```
crates/rlyeh-actor-runtime/
├── Cargo.toml
├── src/
│   ├── lib.rs           ← 入口
│   ├── runtime.rs       ← Runtime + Builder
│   ├── actor.rs         ← ActorState trait + Context
│   ├── mailbox.rs       ← 无锁邮箱
│   ├── scheduler.rs     ← 工作窃取调度
│   ├── supervisor.rs    ← Supervisor + 重启策略
│   ├── envelope.rs      ← 消息信封
│   └── error.rs         ← 错误类型
└── tests/
    └── actor_test.rs
```

---

## 完成后下一步

进入 **P007_增量编译引擎.md**，实现模块级增量编译。
