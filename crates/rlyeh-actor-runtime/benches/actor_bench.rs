//! Actor 运行时基准测试：send / ask / spawn 吞吐。

use criterion::{criterion_group, criterion_main, Criterion};
use zeta_actor_runtime::{ActorContext, ActorError, ActorState, RuntimeBuilder};

struct BenchActor {
    count: u64,
}

impl ActorState for BenchActor {
    fn handle_message(
        &mut self,
        msg: Box<dyn std::any::Any + Send>,
        _ctx: &mut ActorContext,
    ) -> Result<bool, ActorError> {
        if let Some(&m) = msg.downcast_ref::<u64>() {
            self.count += m;
        }
        Ok(false)
    }
}

struct EchoActor;

impl ActorState for EchoActor {
    fn handle_message(
        &mut self,
        msg: Box<dyn std::any::Any + Send>,
        ctx: &mut ActorContext,
    ) -> Result<bool, ActorError> {
        ctx.reply(msg);
        Ok(false)
    }
}

fn bench_send(c: &mut Criterion) {
    let runtime = RuntimeBuilder::new().with_workers(4).build();
    let actor = runtime.spawn(BenchActor { count: 0 });
    c.bench_function("send_100k", |b| {
        b.iter(|| {
            for i in 0..100_000u64 {
                std::hint::black_box(actor.send(Box::new(i))).unwrap();
            }
        });
    });
    runtime.shutdown();
}

fn bench_ask(c: &mut Criterion) {
    let runtime = RuntimeBuilder::new().with_workers(4).build();
    let actor = runtime.spawn(EchoActor);
    c.bench_function("ask_10k", |b| {
        b.iter(|| {
            for i in 0..10_000u64 {
                let _: u64 = std::hint::black_box(actor.ask_blocking(Box::new(i)).unwrap());
            }
        });
    });
    runtime.shutdown();
}

fn bench_spawn(c: &mut Criterion) {
    let runtime = RuntimeBuilder::new().with_workers(4).build();
    c.bench_function("spawn_10k", |b| {
        b.iter(|| {
            for _ in 0..10_000 {
                let _ = std::hint::black_box(runtime.spawn(BenchActor { count: 0 }));
            }
        });
    });
    runtime.shutdown();
}

criterion_group!(benches, bench_send, bench_ask, bench_spawn);
criterion_main!(benches);
