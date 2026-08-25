// actor 定义 + 语言级构造 / ask / send：编译必须成功
actor Counter {
    value: i64 = 0,

    pub fn increment(amount: i64) -> i64 {
        self.value += amount;
        self.value
    }

    pub fn total() -> i64 {
        self.value
    }
}

fn main() {
    let c = Counter::new();
    let r = c.increment(10).await;
    send c.increment(5);
    println(r);
    println(c.total().await);
}
