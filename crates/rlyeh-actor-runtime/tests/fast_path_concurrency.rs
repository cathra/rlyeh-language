//! 回归测试：ask 快速路径（同线程同步短路）与慢路径并发下的正确性。
//! 覆盖多线程并发 ask/send、批量短时间片快速路径、自发送无死锁三类场景。
//! 注意：消息须显式标注 `i64`（裸字面量默认 `i32`，与 downcast_ref::<i64> 不匹配会导致不回复）。

use std::thread;

use rlyeh_actor_runtime::{ActorContext, ActorError, ActorState, RuntimeBuilder};

/// 自增计数器：ask 回复当前值。
struct Counter {
    value: i64,
}

impl ActorState for Counter {
    fn handle_message(
        &mut self,
        msg: Box<dyn std::any::Any + Send>,
        ctx: &mut ActorContext,
    ) -> Result<bool, ActorError> {
        if let Some(&delta) = msg.downcast_ref::<i64>() {
            self.value += delta;
            ctx.reply(self.value);
        }
        Ok(false)
    }
}

#[test]
fn fast_path_concurrent_ask() {
    let runtime = RuntimeBuilder::new().with_workers(2).build();
    let counter = runtime.spawn(Counter { value: 0 });

    let n_threads = 8;
    let n_asks = 2000;
    let mut handles = Vec::new();
    for t in 0..n_threads {
        let c = counter.clone();
        handles.push(thread::spawn(move || {
            for i in 0..n_asks {
                let _: i64 = c.ask_blocking(Box::new(1i64)).unwrap();
                // 少量混合 send 扰动 fast path / 慢路径竞争
                if (t * n_asks + i) % 97 == 0 {
                    let _ = c.send(Box::new(1i64));
                }
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    // 8 线程 × 2000 ask(1) + 扰动 send(1) 总数
    let mut total_sent = n_threads * n_asks;
    for t in 0..n_threads {
        for i in 0..n_asks {
            if (t * n_asks + i) % 97 == 0 {
                total_sent += 1;
            }
        }
    }
    let final_val: i64 = counter.ask_blocking(Box::new(0i64)).unwrap();
    assert_eq!(final_val, total_sent, "并发 ask/send 结果丢失");
    runtime.shutdown();
}

#[test]
fn fast_path_burst_then_drain() {
    // 一批并发 ask 全部走快速路径后，actor 状态仍一致
    let runtime = RuntimeBuilder::new().with_workers(2).build();
    let counter = runtime.spawn(Counter { value: 0 });
    let n_threads = 4;
    let n_asks = 500;
    let mut handles = Vec::new();
    for _ in 0..n_threads {
        let c = counter.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..n_asks {
                let _: i64 = c.ask_blocking(Box::new(1i64)).unwrap();
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    let final_val: i64 = counter.ask_blocking(Box::new(0i64)).unwrap();
    assert_eq!(final_val, (n_threads * n_asks) as i64);
    runtime.shutdown();
}

#[test]
fn fast_path_self_send_no_deadlock() {
    // fast path 处理中向自身 send（fire-and-forget）：由收尾 notify 兜底，
    // 消息不应滞留、进程不应死锁。
    let runtime = RuntimeBuilder::new().with_workers(1).build();
    let a = runtime.spawn(ReplyOnce);
    // 首次 ask：走 fast path（actor 空闲、邮箱空），handler 内 self-send
    let _: i64 = a.ask_blocking(Box::new(0i64)).unwrap();
    // self-send 的消息应已被 Worker 处理（ReplyOnce 会 reply，但 send 无接收方，
    // 仅验证不滞留/不 panic）。再次 ask 验证 actor 仍可用。
    let _: i64 = a.ask_blocking(Box::new(0i64)).unwrap();
    runtime.shutdown();
}

/// 处理消息时向自身 send 一条消息。
struct ReplyOnce;
impl ActorState for ReplyOnce {
    fn handle_message(
        &mut self,
        _msg: Box<dyn std::any::Any + Send>,
        ctx: &mut ActorContext,
    ) -> Result<bool, ActorError> {
        let _ = ctx.self_ref().send(Box::new(99i64));
        ctx.reply(1i64);
        Ok(false)
    }
}
