# Zeta Actor 并发模型规范

> 版本：v2.0  
> 最后更新：2026-08-22

> **⚠️ 实现状态**：本文为**目标规范**。MVP 已实现子集：`actor` 声明（字段/默认值/方法）、
> `Counter::new()` 普通 spawn、`Counter::new_supervised(n)` 监督 spawn（n=0/1/2 对应 OneForOne/
> AllForOne/RestartForOne）、方法调用 `.await`（ask 往返）、`send`（fire-and-forget）、返回 -1 触发
> 崩溃协议（无监督停止 / 监督重启）。**未实现（规划）**：`supervisor {}` 块、`ActorRef<T>`、
> `panic!`/`format!`/`println!` 宏（`!` 为 `not` 运算符）、`dyn Trait` actor 字段、channel、
> `ExitSignal` 监控 API。可运行示例见 [`guide.md`](./guide.md) §9。

## 相关文档

| 类型 | 文档 | 说明 |
|------|------|------|
| 项目总纲 | [CODEBUDDY.md](../CODEBUDDY.md) | 项目全景 |
| 设计文档 | [07_Actor并发模型](../design/07_Actor并发模型.md) | 模块设计 |
| 实现任务 | [P006](../prompts/P006_Actor运行时.md) | Actor 运行时 |

---

## 1. 设计原则

1. **隔离**：每个 Actor 拥有独立状态，不共享内存。
2. **消息传递**：Actor 间仅通过异步消息通信。
3. **单线程执行**：每个 Actor 内部是单线程的，无需锁。
4. **崩溃隔离**：一个 Actor 崩溃不影响其他 Actor。
5. **可恢复**：Supervisor 可以监控并重启崩溃的 Actor。

---

## 2. Actor 定义

### 2.1 基本语法

```zeta
actor Counter {
    // 状态字段（私有，除非加 pub）
    value: u32 = 0,  // 默认值
    name: String = String::new(),
    
    // 初始化方法
    pub fn init(name: String) {
        self.name = name;
    }
    
    // 消息处理方法（公开 = 可被外部调用）
    pub async fn increment(amount: u32) -> u32 {
        self.value += amount;
        self.value
    }
    
    pub async fn get() -> u32 {
        self.value
    }
    
    // 私有方法
    fn reset_internal() {
        self.value = 0;
    }
}
```

### 2.2 状态管理

- Actor 的状态字段在 Actor 创建时分配。
- 状态始终驻留在 Actor 的私有内存空间中。
- 外部只能通过 `pub` 方法访问状态。

### 2.3 生命周期

```
Actor 创建 → 初始化 → 处理消息循环 → 收到停止信号 → 清理 → 销毁
```

---

## 3. 消息传递

### 3.1 发送消息

```zeta
// 创建 Actor 实例
let counter = Counter::new("my-counter");

// 发送消息（异步）
let result = counter.increment(10).await;
println(result);

// 链式调用
let value = counter.increment(5).await;
let final = counter.get().await;
```

### 3.2 消息队列

每个 Actor 有一个 FIFO 消息队列：

```
Counter Actor:
┌─────────────────────────────────┐
│  Message Queue (FIFO)           │
│  ┌───────────────────────────┐  │
│  │ increment(10)             │  │
│  │ get()                     │  │
│  │ increment(5)             │  │
│  │ reset()                   │  │
│  └───────────────────────────┘  │
│                                 │
│  Current State:                  │
│    value = 15                    │
│    name = "my-counter"           │
└─────────────────────────────────┘
```

### 3.3 消息顺序保证

- 同一发送者发送的消息**按序到达**。
- 不同发送者的消息**无全局顺序保证**。
- 如果顺序重要，使用单一发送者或添加序列号。

---

## 4. Actor 运行时

### 4.1 调度模型

```rust
// 运行时伪代码
struct ActorRuntime {
    actors: DashMap<ActorId, ActorHandle>,
    global_queue: WorkStealingQueue<Task>,
    worker_threads: Vec<WorkerThread>,
}

struct ActorHandle {
    mailbox: Mutex<VecDeque<Message>>,
    state: Box<dyn Any>,  // Actor 状态
    vtable: ActorVTable,  // 方法调度表
}
```

### 4.2 工作窃取调度

```
Worker Thread 1:  [Actor A] [Actor B] [Actor C]
Worker Thread 2:  [Actor D] [Actor E]
Worker Thread 3:  [Actor F] [idle]

当 Thread 3 空闲时：
→ 从 Thread 1 的队列窃取一半任务
→ Thread 3 开始处理 Actor A
```

### 4.3 无锁消息队列

```rust
struct LockFreeMailbox {
    // 使用 crossbeam 的 MPMC 队列
    // 发送者无锁，接收者无锁
    queue: crossbeam::queue::ArrayQueue<Message>,
    // 回压机制
    high_watermark: usize,
    low_watermark: usize,
}
```

---

## 5. Supervisor 模型（规划）

> **MVP 现状**：`new_supervised(0|1|2)` + 返回 -1 崩溃协议已实现（runtime 经 `__state_new` 重建初始
> 状态并重启，见 [`CODEBUDDY.md`](../CODEBUDDY.md) §3.3）；下述 `supervisor {}` 声明式语法为规划。

### 5.1 基本语法

```zeta
supervisor {
    strategy: OneForOne,  // 只重启崩溃的子 Actor
    max_restarts: 3,
    within: Duration::seconds(30),
    
    children: [
        Counter::new("counter-1"),
        Counter::new("counter-2"),
        Logger::new("/var/log/app.log"),
    ]
}
```

### 5.2 重启策略

| 策略 | 行为 | 适用场景 |
|------|------|----------|
| `OneForOne` | 只重启崩溃的 Actor | 独立工作单元 |
| `AllForOne` | 一个崩溃，全部重启 | 强耦合的 Actor 组 |
| `RestForOne` | 重启崩溃者及其后续声明的 | 有依赖链的 Actor |

### 5.3 崩溃传播

```
Actor C 崩溃
    │
    ▼
Supervisor 收到 ExitSignal { actor: C, reason: Panic }
    │
    ▼
检查重启策略
    │
    ├── 在时间窗口内重启次数 < max_restarts → 重启 C
    └── 超过限制 → 向上级 Supervisor 报告
```

---

## 6. Actor 间通信模式

### 6.1 请求-响应

```zeta
actor Client {
    server: ActorRef<Server>,
    
    pub async fn fetch_data() -> Result<Data, Error> {
        self.server.request_data().await
    }
}

actor Server {
    db: Database,
    
    pub async fn request_data() -> Result<Data, Error> {
        self.db.query("SELECT * FROM data").await
    }
}
```

### 6.2 发布-订阅

```zeta
actor EventBus {
    subscribers: Vec<ActorRef<dyn Subscriber>> = vec![],
    
    pub fn subscribe(actor: ActorRef<dyn Subscriber>) {
        self.subscribers.push(actor);
    }
    
    pub fn publish(event: Event) {
        for sub in &self.subscribers {
            sub.notify(event.clone());  // 异步发送，不等待
        }
    }
}
```

### 6.3 管道（Pipeline）

```zeta
actor Pipeline {
    stages: Vec<ActorRef<dyn Stage>>,
    
    pub async fn process(input: Data) -> Result<Data, Error> {
        let mut current = input;
        for stage in &self.stages {
            current = stage.transform(current).await?;
        }
        Ok(current)
    }
}
```

---

## 7. 错误处理

### 7.1 Actor 内部错误

```zeta
actor SafeActor {
    pub async fn risky_operation() -> Result<(), ProcessingError> {
        // 如果返回 Err，调用者收到错误
        // Actor 本身不会崩溃
        do_risky_stuff()
    }
}

// 调用方
match actor.risky_operation().await {
    Ok(()) => println!("Success"),
    Err(e) => println!("Failed: {}", e),
}
```

### 7.2 Actor 崩溃

```zeta
actor UnstableActor {
    pub async fn boom() {
        // MVP：方法返回 -1 触发崩溃协议（panic! 宏未实现，属规划）
        return -1;
        // Actor 崩溃，Supervisor 决定是否重启
    }
}

// 监控崩溃
let handle = UnstableActor::new();
let monitor = handle.monitor();  // 返回监控句柄

// 在另一个 Actor 中
pub async fn watch(target: ActorRef) {
    match target.wait_for_exit().await {
        ExitReason::Normal => println!("Graceful shutdown"),
        ExitReason::Panic(msg) => println!("Crashed: {}", msg),
        ExitReason::Killed => println!("Killed by supervisor"),
    }
}
```

---

## 8. 性能模型

### 8.1 消息传递开销

| 操作 | 延迟 | 说明 |
|------|------|------|
| 发送消息（无等待） | ~50 ns | 入队 + 通知 |
| 发送 + 等待回复 | ~200 ns | 包含 future 唤醒 |
| 跨线程发送 | ~100 ns | 工作窃取开销 |
| 批量发送 | ~30 ns/msg | 批处理优化 |

### 8.2 吞吐量目标

| 场景 | 目标吞吐量 |
|------|------------|
| 空消息传递 | > 10M msg/s |
| 小消息（< 64 bytes） | > 5M msg/s |
| 大消息（1KB） | > 1M msg/s |
| Actor 创建/销毁 | > 100K actors/s |

### 8.3 内存开销

| 组件 | 每 Actor 开销 |
|------|----------------|
| Actor 句柄 | 64 bytes |
| 消息队列（空） | 256 bytes（预分配） |
| 状态（空） | 取决于字段 |
| 监控引用 | 32 bytes |

---

## 9. 与其他并发原语的互操作

### 9.1 与线程的关系

```zeta
// Actor 运行在线程池之上
// 一个 Worker Thread 可以调度多个 Actor
// Actor 不直接绑定到特定线程

// 但你可以 pin Actor 到特定线程
actor PinnedActor {
    #[pinned(thread = 2)]  // 绑定到线程 2
    pub fn process(data: Data) {
        // 保证在同一线程执行
    }
}
```

### 9.2 与 Channel 的互操作

```zeta
// Actor 可以暴露 channel 接口
actor ChannelBridge {
    sender: Sender<Message>,
    receiver: Receiver<Message>,
    
    pub fn from_channel(sender: Sender<Message>) -> Self {
        Self { sender, receiver: create_receiver() }
    }
}
```

### 9.3 与传统锁的互操作

```zeta
// Actor 内部不需要锁（单线程执行）
// 但可以与外部共享资源交互
actor FileWriter {
    // 使用 Mutex 保护外部资源
    file: Mutex<File>,
    
    pub fn write(data: &[u8]) {
        let mut file = self.file.lock();
        file.write_all(data);
    }
}
```

---

## 10. 完整示例（目标示例，含规划语法）

> 下述 `ActorRef`/`supervisor {}`/`format!` 等为规划语法；MVP 可运行版本见
> [`guide.md`](./guide.md) §9.2（监督计数示例）。

```zeta
// Chat Room 示例
actor ChatRoom {
    name: String,
    users: HashMap<UserId, ActorRef<User>> = HashMap::new(),
    history: Vec<Message> = vec![],
    
    pub fn join(user_id: UserId, user: ActorRef<User>) {
        self.users.insert(user_id, user);
        self.broadcast(SystemMsg(format!("User {} joined", user_id)));
    }
    
    pub fn leave(user_id: UserId) {
        self.users.remove(&user_id);
        self.broadcast(SystemMsg(format!("User {} left", user_id)));
    }
    
    pub fn send(sender: UserId, text: String) {
        let msg = Message { from: sender, text, timestamp: now() };
        self.history.push(msg.clone());
        self.broadcast(msg);
    }
    
    fn broadcast(msg: Message) {
        for user in self.users.values() {
            user.deliver(msg.clone());
        }
    }
}

// Supervisor 管理聊天室
supervisor {
    strategy: OneForOne,
    max_restarts: 5,
    within: Duration::minutes(5),
    
    children: [
        ChatRoom::new("General"),
        ChatRoom::new("Random"),
        ChatRoom::new("Tech"),
    ]
}
```

---

> **维护者**：Zeta Language Team  
> **License**：MIT / Apache-2.0
