> [!NOTE]
> 本文档为早期设计稿（项目 bootstrap 阶段），已归档至 `docs/design/`。
> 权威规范见 `docs/` 目录对应文件，映射关系见 `docs/design/README.md`。

# 07 Actor 并发模型

## 相关文档

| 类型 | 文档 | 说明 |
|------|------|------|
| 并发规范 | [docs/actor-model.md](../actor-model.md) | Actor 模型规范 |
| 实现任务 | [prompts/P006_Actor运行时.md](./prompts/P006_Actor运行时.md) | Actor 运行时实现 |

## 目标

在 Rlyeh 语言层面内置 Actor 模型，让并发编程像写单线程代码一样简单。

## Actor 语法

```rlyeh
actor Counter {
    // 状态（私有字段）
    value: u32 = 0,
    name: String,
    
    // 初始化
    pub fn init(name: String) -> Self {
        Self { value: 0, name }
    }
    
    // 消息处理方法（公开）
    pub fn increment(amount: u32) -> u32 {
        self.value += amount;
        self.value
    }
    
    pub fn get() -> u32 {
        self.value
    }
    
    pub fn reset() {
        self.value = 0;
    }
    
    // 私有方法
    fn log(&self, msg: &str) {
        println!("[{}] {}", self.name, msg);
    }
}

// 使用
let counter = Counter::init("my_counter");
let result = counter.increment(10).await;  // 发送消息并等待回复
println!("Counter: {}", result);
```

## Actor 运行时设计

### 核心数据结构

```rust
/// Actor 实例
pub struct Actor<T: ActorState> {
    /// 消息队列（无锁 MPSC）
    mailbox: MpscQueue<Envelope>,
    /// Actor 状态（私有）
    state: T,
    /// 运行时引用
    runtime: RuntimeRef,
    /// Actor ID
    id: ActorId,
}

/// 消息信封
pub struct Envelope {
    /// 消息类型标识
    msg_type: TypeId,
    /// 消息数据（堆分配）
    data: Box<dyn Any>,
    /// 回复通道
    reply_to: Option<ReplyChannel>,
    /// 消息 ID
    msg_id: u64,
}

/// Actor 运行时
pub struct ActorRuntime {
    /// 工作线程池
    workers: Vec<WorkerThread>,
    /// 全局 Actor 注册表
    registry: ActorRegistry,
    /// 调度器
    scheduler: Scheduler,
}

/// 调度器
pub struct Scheduler {
    /// 就绪队列
    ready: WorkStealingQueue<ActorId>,
    /// 等待中的 Actor（等待消息）
    waiting: HashSet<ActorId>,
    /// 统计信息
    stats: SchedulerStats,
}
```

### 消息发送与接收

```rust
impl<T: ActorState> Actor<T> {
    /// 发送消息（异步）
    pub async fn send<M: Message>(&self, msg: M) -> Result<M::Reply, SendError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        
        let envelope = Envelope {
            msg_type: TypeId::of::<M>(),
            data: Box::new(msg),
            reply_to: Some(ReplyChannel::new(reply_tx)),
            msg_id: self.runtime.next_msg_id(),
        };
        
        // 放入邮箱
        self.mailbox.push(envelope);
        
        // 通知调度器
        self.runtime.notify_actor_ready(self.id);
        
        // 等待回复
        reply_rx.await.map_err(|_| SendError::ActorDied)
    }
    
    /// Actor 主循环
    pub async fn run(mut self) {
        loop {
            // 等待消息
            let envelope = self.mailbox.pop().await;
            
            // 动态分发
            let result = self.dispatch(envelope).await;
            
            // 发送回复
            if let Some(reply) = result {
                envelope.reply_to.unwrap().send(reply);
            }
        }
    }
}
```

### 动态分发

```rust
/// 每个 Actor 类型自动生成 dispatch 方法
impl Counter {
    async fn dispatch(&mut self, envelope: Envelope) -> Option<Box<dyn Any>> {
        match envelope.msg_type {
            t if t == TypeId::of::<IncrementMsg>() => {
                let msg: IncrementMsg = *envelope.data.downcast().unwrap();
                let result = self.increment(msg.amount);
                Some(Box::new(result))
            }
            t if t == TypeId::of::<GetMsg>() => {
                let result = self.get();
                Some(Box::new(result))
            }
            t if t == TypeId::of::<ResetMsg>() => {
                self.reset();
                Some(Box::new(()))
            }
            _ => panic!("Unknown message type"),
        }
    }
}
```

## 编译器代码生成

### Actor 结构体生成

```rlyeh
// 源码
actor Counter {
    value: u32 = 0,
    pub fn increment(amount: u32) -> u32 { ... }
}
```

```rust
// 编译器生成（伪代码）

// 1. 状态结构体
#[repr(C)]
struct CounterState {
    value: u32,
}

// 2. 消息类型定义
struct IncrementMsg { amount: u32 }
struct GetMsg {}
struct ResetMsg {}

// 3. Reply 类型
type IncrementReply = u32;
type GetReply = u32;
type ResetReply = ();

// 4. Actor 包装类型
struct Counter {
    id: ActorId,
    mailbox: ActorRef,
}

impl Counter {
    pub fn init(name: String) -> Self {
        let state = CounterState { value: 0 };
        let (actor, mailbox) = spawn_actor(state);
        Counter { id: actor.id(), mailbox }
    }
    
    pub async fn increment(&self, amount: u32) -> u32 {
        let msg = IncrementMsg { amount };
        self.mailbox.send(msg).await.unwrap()
    }
    
    pub async fn get(&self) -> u32 {
        let msg = GetMsg {};
        self.mailbox.send(msg).await.unwrap()
    }
    
    pub async fn reset(&self) {
        let msg = ResetMsg {};
        self.mailbox.send(msg).await.unwrap()
    }
}

// 5. Actor 主循环（编译器生成）
async fn counter_main(mut state: CounterState, mailbox: MpscQueue<Envelope>) {
    loop {
        let envelope = mailbox.pop().await;
        // ... dispatch 逻辑
    }
}
```

## Supervisor 机制

```rlyeh
// Supervisor 定义
supervisor DatabaseSupervisor {
    // 监控的子 Actor
    children: Vec<ActorRef>,
    
    // 重启策略
    strategy: RestartStrategy = OneForOne,
    
    // 最大重启次数
    max_restarts: u32 = 5,
    within: Duration = 1.minute(),
    
    fn on_child_crashed(child: ActorId, error: ActorError) {
        match self.strategy {
            OneForOne => self.restart(child),
            OneForAll => self.restart_all(),
            RestForOne => self.restart(child_and_descendants),
        }
    }
}

// 重启策略
enum RestartStrategy {
    /// 只重启崩溃的 Actor
    OneForOne,
    /// 重启所有子 Actor
    OneForAll,
    /// 重启崩溃的 Actor 及其依赖
    RestForOne,
}
```

## 工作窃取调度器

```rust
pub struct WorkerThread {
    /// 本地队列
    local_queue: VecDeque<ActorId>,
    /// 偷取目标
    steal_targets: Vec<usize>,
    /// 当前运行的 Actor
    current: Option<ActorId>,
    /// 线程 ID
    thread_id: usize,
}

impl WorkerThread {
    pub fn run(&mut self) {
        loop {
            // 1. 优先处理本地队列
            if let Some(actor) = self.local_queue.pop_front() {
                self.process_actor(actor);
                continue;
            }
            
            // 2. 尝试偷取
            if let Some(actor) = self.steal() {
                self.local_queue.push_back(actor);
                continue;
            }
            
            // 3. 全局队列
            if let Some(actor) = self.runtime.global_queue.pop() {
                self.local_queue.push_back(actor);
                continue;
            }
            
            // 4. 休眠等待
            self.park();
        }
    }
    
    fn steal(&mut self) -> Option<ActorId> {
        for &target in &self.steal_targets {
            if let Some(actor) = self.runtime.workers[target]
                .local_queue
                .steal_half()
            {
                return Some(actor);
            }
        }
        None
    }
}
```

## 与所有权系统的集成

### Actor 间消息的所有权转移

```rlyeh
actor Producer {
    pub fn produce() -> Data {
        let data = Data::new();  // 在 Producer 的区域内分配
        data  // 所有权转移到消费者
    }
}

actor Consumer {
    pub fn consume(data: Data) {
        // data 的所有权已转移到 Consumer
        process(data);
    }
}

// 使用
let producer = Producer::init();
let consumer = Consumer::init();

let data = producer.produce().await;
consumer.consume(data).await;  // 所有权转移
```

### 编译器检查

```rust
/// 消息类型必须实现 Send
/// （因为消息在线程间传递）
pub trait Message: Send + 'static {
    type Reply: Send + 'static;
}

/// Actor 状态必须实现 ActorState
pub trait ActorState: Send + 'static {
    fn dispatch(&mut self, envelope: Envelope) -> Pin<Box<dyn Future<Output = _> + Send>>;
}
```

## 性能目标

| 指标 | 目标 |
|------|------|
| Actor 创建开销 | < 1μs |
| 消息发送延迟（同线程） | < 50ns |
| 消息发送延迟（跨线程） | < 200ns |
| 100 万 Actor 内存占用 | < 500MB |
| 上下文切换开销 | < 100ns |

## 交付物

- `actor.rs`：Actor 核心 trait 和类型
- `mailbox.rs`：无锁消息队列
- `scheduler.rs`：工作窃取调度器
- `supervisor.rs`：Supervisor 实现
- `codegen.rs`：Actor 代码生成
- `dispatch.rs`：动态消息分发
- `test_actor.rs`：单元测试
- `bench_actor.rs`：性能基准

## 验收标准

1. Actor 间消息传递类型安全
2. 消息所有权正确转移
3. Supervisor 重启策略正确
4. 工作窃取有效平衡负载
5. 100 万消息/秒吞吐量
6. 死锁检测和报告
