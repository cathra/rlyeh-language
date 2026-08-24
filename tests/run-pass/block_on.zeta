// block_on.zeta：S1 异步运行时基础（Poll + Future trait + block_on 手动轮询）
// S1a：`Poll::Ready`/`Poll::Pending` 构造与解构；
// S1b：block_on 循环轮询到 Ready，返回携带值。
struct MyFut {
    state: i64,
}

impl Future for MyFut {
    fn poll(&mut self) -> Poll<i64> {
        self.state += 1;
        if self.state >= 3 {
            Poll::Ready(self.state)
        } else {
            Poll::Pending
        }
    }
}

fn main() {
    // S1a：直接构造/解构 Poll 枚举（Ready 携带值）
    match Poll::Ready(42) {
        Poll::Ready(v) => println(v),
        Poll::Pending => println(0),
    }
    // S1b：block_on 手动轮询——前两轮 Pending，第三轮 Ready(3)
    let mut fut = MyFut { state: 0 };
    let v = block_on(&mut fut);
    println(v);
}
