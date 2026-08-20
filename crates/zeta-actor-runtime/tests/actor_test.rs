//! zeta-actor-runtime 集成测试。
//!
//! 覆盖 P006 定义的验收用例：基本消息、ask 模式、Supervisor 重启、
//! 消息顺序、崩溃隔离、优雅关闭、spawn 链、高吞吐，以及内置
//! Router / Timer 与监督决策的补充用例。

use std::any::Any;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use zeta_actor_runtime::{
    ActorContext, ActorError, ActorId, ActorRef, ActorState, ActorStatus, RestartStrategy,
    Router, RouterMsg, RuntimeBuilder, Supervisor, SupervisorDecision, SupervisorStrategy,
};

// ---------- 测试辅助：通用 Actor ----------

/// 计数器：`i64` 增量 + 查询回复。
struct Counter {
    value: i64,
}

/// 查询当前值。
struct GetValue;

/// 通用查询标记。
struct Query;

impl ActorState for Counter {
    fn handle_message(
        &mut self,
        msg: Box<dyn Any + Send>,
        ctx: &mut ActorContext,
    ) -> Result<bool, ActorError> {
        if let Some(&delta) = msg.downcast_ref::<i64>() {
            self.value += delta;
        }
        if msg.downcast_ref::<GetValue>().is_some() {
            ctx.reply(self.value);
        }
        Ok(false)
    }
}

/// 原样回复消息的 Echo。
struct Echo;

impl ActorState for Echo {
    fn handle_message(
        &mut self,
        msg: Box<dyn Any + Send>,
        ctx: &mut ActorContext,
    ) -> Result<bool, ActorError> {
        ctx.reply_boxed(msg);
        Ok(false)
    }
}

/// 每处理一条消息就崩溃一次的占位。
struct AlwaysCrash;

impl ActorState for AlwaysCrash {
    fn handle_message(
        &mut self,
        _msg: Box<dyn Any + Send>,
        _ctx: &mut ActorContext,
    ) -> Result<bool, ActorError> {
        Err(ActorError::Panic {
            reason: "always crash".into(),
        })
    }
}

// ---------- 测试用例 ----------

#[test]
fn test_basic_send() {
    let runtime = RuntimeBuilder::new().with_workers(2).build();
    let counter: ActorRef = runtime.spawn(Counter { value: 0 });
    for i in 1..=10 {
        // 注意：`1..=10` 的 `i` 默认是 `i32`，必须显式转为 `i64` 才能被 Counter 匹配
        counter.send(Box::new(i as i64)).unwrap();
    }
    // ask 保证此前 FIFO 消息全部处理完
    let v: i64 = counter.ask_blocking(Box::new(GetValue)).unwrap();
    assert_eq!(v, 55);
    runtime.shutdown();
}

#[test]
fn test_ask_pattern() {
    let runtime = RuntimeBuilder::new().with_workers(2).build();
    let echo = runtime.spawn(Echo);
    let r: i64 = echo.ask_blocking(Box::new(42i64)).unwrap();
    assert_eq!(r, 42);
    let s: String = echo.ask_blocking(Box::new("hi".to_string())).unwrap();
    assert_eq!(s, "hi");
    // 回复类型不匹配
    let bad: Result<i32, ActorError> = echo.ask_blocking(Box::new(1i64));
    assert!(matches!(bad, Err(ActorError::WrongReplyType)));
    runtime.shutdown();
}

#[test]
fn test_supervisor_restart() {
    static PROCESSED: AtomicUsize = AtomicUsize::new(0);
    PROCESSED.store(0, Ordering::SeqCst);

    struct Flaky {
        processed: Arc<AtomicUsize>,
    }
    impl ActorState for Flaky {
        fn handle_message(
            &mut self,
            _msg: Box<dyn Any + Send>,
            _ctx: &mut ActorContext,
        ) -> Result<bool, ActorError> {
            let n = self.processed.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                return Err(ActorError::Panic {
                    reason: "first message always crashes".into(),
                });
            }
            Ok(false)
        }
    }

    let processed = Arc::new(AtomicUsize::new(0));
    let factory = {
        let processed = processed.clone();
        move || Box::new(Flaky { processed: processed.clone() }) as Box<dyn ActorState>
    };
    let runtime = RuntimeBuilder::new().with_workers(1).build();
    let flaky = runtime.spawn_supervised(factory, RestartStrategy::OneForOne);
    for _ in 0..3 {
        flaky.send(Box::new(1i64)).unwrap();
    }
    // 轮询等待全部处理（第 1 条崩溃后重启，第 2、3 条成功）
    let deadline = Instant::now() + Duration::from_secs(5);
    while processed.load(Ordering::SeqCst) < 3 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(processed.load(Ordering::SeqCst), 3);
    // Actor 已重启并存活（保留原 ID）
    assert_ne!(flaky.status(), ActorStatus::Stopped);
    runtime.shutdown();
}

#[test]
fn test_supervisor_restart_limit() {
    let runtime = RuntimeBuilder::new().with_workers(1).build();
    let actor = runtime.spawn_supervised(
        || Box::new(AlwaysCrash) as Box<dyn ActorState>,
        RestartStrategy::OneForOne,
    );
    // 超过默认 max_restarts(10) 后应停止
    for _ in 0..25 {
        let _ = actor.send(Box::new(1i64));
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while actor.status() != ActorStatus::Stopped && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(actor.status(), ActorStatus::Stopped);
    runtime.shutdown();
}

#[test]
fn test_message_ordering() {
    struct OrderTracker {
        last: i64,
        ok: bool,
    }
    struct CheckOrder;
    impl ActorState for OrderTracker {
        fn handle_message(
            &mut self,
            msg: Box<dyn Any + Send>,
            ctx: &mut ActorContext,
        ) -> Result<bool, ActorError> {
            if let Some(&v) = msg.downcast_ref::<i64>() {
                if v != self.last + 1 {
                    self.ok = false;
                }
                self.last = v;
            }
            if msg.downcast_ref::<CheckOrder>().is_some() {
                ctx.reply(self.ok);
            }
            Ok(false)
        }
    }

    let runtime = RuntimeBuilder::new().with_workers(4).build();
    let tracker = runtime.spawn(OrderTracker { last: 0, ok: true });
    for i in 1..=1000 {
        tracker.send(Box::new(i as i64)).unwrap();
    }
    let ok: bool = tracker.ask_blocking(Box::new(CheckOrder)).unwrap();
    assert!(ok, "messages processed out of order");
    runtime.shutdown();
}

#[test]
fn test_actor_isolation() {
    struct Crash;
    impl ActorState for Crash {
        fn handle_message(
            &mut self,
            _msg: Box<dyn Any + Send>,
            _ctx: &mut ActorContext,
        ) -> Result<bool, ActorError> {
            Err(ActorError::Panic {
                reason: "boom".into(),
            })
        }
    }

    let runtime = RuntimeBuilder::new().with_workers(2).build();
    let steady: ActorRef = runtime.spawn(Counter { value: 0 });
    let crash = runtime.spawn(Crash);
    let _ = crash.send(Box::new(1i64));
    // 等 crash actor 处理（崩溃、无监督 → 停止）
    let deadline = Instant::now() + Duration::from_secs(5);
    while crash.status() != ActorStatus::Stopped && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(crash.status(), ActorStatus::Stopped);
    // steady 不受影响
    let _ = steady.send(Box::new(5i64));
    let v: i64 = steady.ask_blocking(Box::new(GetValue)).unwrap();
    assert_eq!(v, 5);
    runtime.shutdown();
}

#[test]
fn test_graceful_shutdown() {
    struct Slow {
        processed: Arc<AtomicUsize>,
    }
    impl ActorState for Slow {
        fn handle_message(
            &mut self,
            _msg: Box<dyn Any + Send>,
            _ctx: &mut ActorContext,
        ) -> Result<bool, ActorError> {
            std::thread::sleep(Duration::from_millis(1));
            self.processed.fetch_add(1, Ordering::SeqCst);
            Ok(false)
        }
    }

    let processed = Arc::new(AtomicUsize::new(0));
    let runtime = RuntimeBuilder::new().with_workers(1).build();
    let actor = runtime.spawn(Slow {
        processed: processed.clone(),
    });
    for _ in 0..50 {
        actor.send(Box::new(1u8)).unwrap();
    }
    runtime.shutdown();
    // 优雅关闭应排空全部消息（每条 sleep 1ms，共约 50ms 处理时间）。
    // 注意：不断言耗时下限——并发环境下测试线程的发送节奏不定，
    // worker 可能已提前处理完，时间断言会负载敏感地闪烁失败。
    assert_eq!(processed.load(Ordering::SeqCst), 50);
}

#[test]
fn test_spawn_chain() {
    struct Child {
        value: i64,
    }
    impl ActorState for Child {
        fn handle_message(
            &mut self,
            msg: Box<dyn Any + Send>,
            ctx: &mut ActorContext,
        ) -> Result<bool, ActorError> {
            if let Some(&v) = msg.downcast_ref::<i64>() {
                self.value += v;
            }
            if msg.downcast_ref::<GetValue>().is_some() {
                ctx.reply(self.value);
            }
            Ok(false)
        }
    }

    struct Parent;
    impl ActorState for Parent {
        fn handle_message(
            &mut self,
            msg: Box<dyn Any + Send>,
            ctx: &mut ActorContext,
        ) -> Result<bool, ActorError> {
            if let Some(&()) = msg.downcast_ref::<()>() {
                let child_id = ctx
                    .spawn(Box::new(Child { value: 0 }))
                    .expect("spawn child");
                let _ = ctx.send(child_id, Box::new(10i64));
                ctx.reply(child_id);
            }
            Ok(false)
        }
    }

    let runtime = RuntimeBuilder::new().with_workers(2).build();
    let parent = runtime.spawn(Parent);
    let child_id: ActorId = parent.ask_blocking(Box::new(())).unwrap();
    let child = runtime.resolve(child_id);
    // 等待父发来的 10 处理完
    let v: i64 = child.ask_blocking(Box::new(GetValue)).unwrap();
    assert_eq!(v, 10);
    runtime.shutdown();
}

#[test]
fn test_high_throughput() {
    struct Accum {
        total: i64,
    }
    impl ActorState for Accum {
        fn handle_message(
            &mut self,
            msg: Box<dyn Any + Send>,
            ctx: &mut ActorContext,
        ) -> Result<bool, ActorError> {
            if let Some(&v) = msg.downcast_ref::<i64>() {
                self.total += v;
            }
            if msg.downcast_ref::<Query>().is_some() {
                ctx.reply(self.total);
            }
            Ok(false)
        }
    }

    const N: i64 = 25_000;
    const THREADS: i64 = 4;
    let runtime = RuntimeBuilder::new().with_workers(4).build();
    let acc = runtime.spawn(Accum { total: 0 });
    let mut handles = Vec::new();
    for _ in 0..THREADS {
        let acc = acc.clone();
        handles.push(std::thread::spawn(move || {
            for i in 0..N {
                // fire-and-forget 无背压：邮箱满时退避重试，保证全部送达
                loop {
                    match acc.send(Box::new(i)) {
                        Ok(()) => break,
                        Err(ActorError::MailboxFull) => std::thread::yield_now(),
                        Err(e) => panic!("send failed: {e:?}"),
                    }
                }
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    let expected = THREADS * (N - 1) * N / 2;
    let v: i64 = acc.ask_blocking(Box::new(Query)).unwrap();
    assert_eq!(v, expected);
    runtime.shutdown();
}

#[test]
fn test_supervisor_all_for_one() {
    struct CrashMsg;
    struct GroupActor {
        count: i64,
        crash_flag: Arc<AtomicBool>,
    }
    impl ActorState for GroupActor {
        fn handle_message(
            &mut self,
            msg: Box<dyn Any + Send>,
            ctx: &mut ActorContext,
        ) -> Result<bool, ActorError> {
            if msg.downcast_ref::<CrashMsg>().is_some() {
                // 只有第一个触发的崩溃（共享 flag 保证只崩一次）
                if self.crash_flag.swap(false, Ordering::SeqCst) {
                    return Err(ActorError::Panic {
                        reason: "group crash".into(),
                    });
                }
            }
            if let Some(&v) = msg.downcast_ref::<i64>() {
                self.count += v;
            }
            if msg.downcast_ref::<Query>().is_some() {
                ctx.reply(self.count);
            }
            Ok(false)
        }
    }

    let crash_flag = Arc::new(AtomicBool::new(true));
    let factory = {
        let flag = crash_flag.clone();
        move || {
            Box::new(GroupActor {
                count: 0,
                crash_flag: flag.clone(),
            }) as Box<dyn ActorState>
        }
    };
    let runtime = RuntimeBuilder::new().with_workers(2).build();
    let group = runtime.spawn_group(factory, RestartStrategy::AllForOne, 3);
    // 触发一次崩溃 → 全组重启
    for r in &group {
        r.send(Box::new(CrashMsg)).unwrap();
    }
    // 等崩溃 / 重启完成（重启后 count 归零）
    std::thread::sleep(Duration::from_millis(50));
    // 各发增量并验证：重启后 count 从 0 开始，但 CrashMsg 已消费
    for r in &group {
        r.send(Box::new(10i64)).unwrap();
    }
    for r in &group {
        let v: i64 = r.ask_blocking(Box::new(Query)).unwrap();
        assert_eq!(v, 10, "actor should be restarted and start fresh");
    }
    runtime.shutdown();
}

#[test]
fn test_restart_for_one_decision() {
    // 单元级验证 Supervisor 决策逻辑（不依赖调度时序）
    let sup = Supervisor::new();
    let ids: Vec<ActorId> = (0..3).map(|_| ActorId::new()).collect();
    let factory: Arc<dyn Fn() -> Box<dyn ActorState> + Send + Sync> =
        Arc::new(|| Box::new(Echo) as Box<dyn ActorState>);
    let strat = SupervisorStrategy {
        strategy: RestartStrategy::RestartForOne,
        max_restarts: 10,
        within: Duration::from_secs(30),
        children: ids.clone(),
        factory,
    };
    for &c in &ids {
        sup.register(c, strat.clone());
    }
    let err = ActorError::Panic {
        reason: "boom".into(),
    };
    match sup.handle_crash(ids[1], err) {
        SupervisorDecision::Restart(v) => {
            // 崩溃者及其之后声明的兄弟
            assert_eq!(v, vec![ids[1], ids[2]]);
        }
        _ => panic!("expected Restart decision"),
    }
    // 无监督信息 → Escalate
    match sup.handle_crash(ActorId::new(), ActorError::Panic { reason: "x".into() }) {
        SupervisorDecision::Escalate(_) => {}
        _ => panic!("expected Escalate for unregistered actor"),
    }
}

#[test]
fn test_builtin_router() {
    let runtime = RuntimeBuilder::new().with_workers(2).build();
    let a: ActorRef = runtime.spawn(Counter { value: 0 });
    let b: ActorRef = runtime.spawn(Counter { value: 0 });
    let mut router = Router::new();
    router.add_route("a", a.id());
    router.add_route("b", b.id());
    let router = runtime.spawn(router);

    router
        .send(Box::new(RouterMsg {
            key: "a".into(),
            payload: Box::new(5i64),
        }))
        .unwrap();
    router
        .send(Box::new(RouterMsg {
            key: "b".into(),
            payload: Box::new(7i64),
        }))
        .unwrap();

    // 轮询等待路由转发完成
    let deadline = Instant::now() + Duration::from_secs(5);
    let (mut va, mut vb) = (0i64, 0i64);
    while Instant::now() < deadline {
        va = a.ask_blocking(Box::new(GetValue)).unwrap();
        vb = b.ask_blocking(Box::new(GetValue)).unwrap();
        if va == 5 && vb == 7 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(va, 5);
    assert_eq!(vb, 7);
    runtime.shutdown();
}

#[test]
fn test_builtin_timer() {
    let ticks = Arc::new(AtomicUsize::new(0));
    let runtime = RuntimeBuilder::new().with_workers(2).build();
    let t = ticks.clone();
    let _timer = runtime.spawn_timer(
        Duration::from_millis(20),
        Box::new(move || {
            t.fetch_add(1, Ordering::SeqCst);
        }),
    );
    std::thread::sleep(Duration::from_millis(120));
    let n = ticks.load(Ordering::SeqCst);
    assert!(n >= 2, "expected at least 2 ticks, got {n}");
    runtime.shutdown();
}

#[test]
fn test_stop_actor() {
    let runtime = RuntimeBuilder::new().with_workers(1).build();
    let actor = runtime.spawn(Counter { value: 0 });
    actor.send(Box::new(1i64)).unwrap();
    actor.stop();
    let deadline = Instant::now() + Duration::from_secs(5);
    while actor.status() != ActorStatus::Stopped && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(actor.status(), ActorStatus::Stopped);
    // 停止后发送失败
    assert!(matches!(
        actor.send(Box::new(2i64)),
        Err(ActorError::ActorStopped)
    ));
    runtime.shutdown();
}

#[test]
fn test_context_send_between_actors() {
    struct Relayer {
        target: ActorId,
    }
    impl ActorState for Relayer {
        fn handle_message(
            &mut self,
            msg: Box<dyn Any + Send>,
            ctx: &mut ActorContext,
        ) -> Result<bool, ActorError> {
            // 将收到的消息转发给目标
            ctx.send(self.target, msg).unwrap();
            Ok(false)
        }
    }

    let runtime = RuntimeBuilder::new().with_workers(2).build();
    let sink: ActorRef = runtime.spawn(Counter { value: 0 });
    let relayer = runtime.spawn(Relayer {
        target: sink.id(),
    });
    relayer.send(Box::new(3i64)).unwrap();
    relayer.send(Box::new(4i64)).unwrap();
    // 轮询等待转发处理完成（避免与 sink 的查询竞态）
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut v = 0i64;
    while Instant::now() < deadline {
        v = sink.ask_blocking(Box::new(GetValue)).unwrap();
        if v == 7 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(v, 7);
    runtime.shutdown();
}
