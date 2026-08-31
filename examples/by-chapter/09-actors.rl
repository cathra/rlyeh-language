// 对应 docs/guide/09-actors.md —— Actor 并发模型
// 运行：rlyeh run 09-actors.rl
actor Counter {
    value: i64 = 0,
    pub fn increment(amount: i64) -> i64 {
        self.value += amount;
        self.value
    }
    // 练习 2：返回 -1 触发崩溃信号，受监督 actor 会重建初始状态并重启
    pub fn crash_probe() -> i64 {
        -1
    }
}

fn main() {
    let c = Counter::new_supervised(0);   // OneForOne 监督
    println(c.increment(5).await);        // 5：ask 同步往返
    println(c.increment(3).await);        // 8
}
