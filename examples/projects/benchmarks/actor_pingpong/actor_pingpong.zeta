// 基准: actor_pingpong —— 5 万次 actor 同步往返（ask）
// 测: 消息传递 + actor 运行时调度（Zeta 并发模型核心卖点）
// 逻辑: main 循环发送 ping(i)，actor 收到后 count+1 并回传 count，
//       main 累加全部回传值。输出 = Σ(1..50000) = 1,250,025,000
actor Pong {
    count: i64 = 0,

    pub fn ping(x: i64) -> i64 {
        self.count = self.count + 1;
        self.count
    }
}

fn main() {
    let p = Pong::new();
    let mut sum = 0;
    let mut i = 0;
    while i < 50000 {
        let r = p.ping(i).await;
        sum = sum + r;
        i = i + 1;
    }
    println(sum);
}
