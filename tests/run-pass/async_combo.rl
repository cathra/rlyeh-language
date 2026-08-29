// W 阶段组合验收：W1–W5 + U7/U8 能力协同
// - 泛型结构体 Output future（U8 + W4 F::Output）+ join_all（W4）
// - block_on 泛型化返回 String（W4）
// - timeout 泛型化返回 Result<String, TimeoutError>（W4）+ 事件驱动 sleep（W3）
// - recv_async 真异步（W5）+ block_on

struct GenOut<T> { v: T, n: i64 }
impl<T> Future for GenOut<T> {
    type Output = T;
    fn poll(&mut self, cx: &mut Context) -> Poll<T> {
        self.n = self.n + 1;
        if self.n >= 2 {
            Poll::Ready(self.v)
        } else {
            Poll::Pending
        }
    }
}

fn main() {
    // 1. join_all 收集 Vec<String>（泛型结构体 Output）
    let fs: Vec<GenOut<String>> = vec![
        GenOut<String> { v: String::from("a"), n: 0 },
        GenOut<String> { v: String::from("b"), n: 0 },
    ];
    let r = future::join_all(fs);
    for x in r {
        println(x);          // a b
    }
    // 2. block_on 泛型化返回 String
    let mut d = GenOut<String> { v: String::from("zz"), n: 0 };
    let s = block_on(&mut d);
    println(s);              // zz
    // 3. timeout 泛型化返回 Result<String, TimeoutError>（成功路径）
    let mut d2 = GenOut<String> { v: String::from("tt"), n: 0 };
    match timeout(Duration::milliseconds(100), &mut d2) {
        Result::Ok(v) => println(v),     // tt
        Result::Err(e) => println("err"),
    }
    // 4. recv_async 真异步（W5）+ block_on
    let mut p = channel::<i64>();
    let mut tx = p.tx;
    let mut rx = p.rx;
    tx.send(42);
    // P7c：RecvAsync<T> 泛型化 + `Output = Option<T>`
    let mut ra: sync::RecvAsync<i64> = rx.recv_async();
    match block_on(&mut ra) {
        Option::Some(rv) => println(rv), // 42
        Option::None => println(-1),
    }
}
