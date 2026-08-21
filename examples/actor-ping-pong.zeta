// actor-ping-pong.zeta — Actor ping-pong 示例
//
// 演示：
// - `Pong::new()` 语言级构造（编译器生成 __state_new + zeta_actor_spawn 调用）
// - `p.ping(x).await` ask 同步往返（方法经消息槽传参）
// - `send p.ping(x)` 异步 fire-and-forget（不等待结果）
// - 同一 actor 消息按 FIFO 顺序处理，send 后立即 ask 能看到累积状态

actor Pong {
    hits: i64 = 0,

    // ping：hits + 1，返回 x + 1
    pub fn ping(x: i64) -> i64 {
        self.hits += 1;
        x + 1
    }

    // 读取累计接球次数
    pub fn hits() -> i64 {
        self.hits
    }
}

fn main() {
    let p = Pong::new();
    // 同步往返（ask）
    let r1 = p.ping(10).await;
    let r2 = p.ping(r1).await;
    println(r1);
    println(r2);
    println(p.hits().await);
    // 异步发送（send），不等待结果；消息按序入队
    send p.ping(100);
    send p.ping(200);
    // send 之后立刻 ask，FIFO 保证前两条已处理
    println(p.hits().await);
}
