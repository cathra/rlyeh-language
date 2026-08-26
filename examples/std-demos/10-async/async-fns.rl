// S1c async fn/await：状态机 desugar
// - `async fn` desugar 为 Future 结构体 + poll 状态机 + 构造器（返回 i64 / ()）
// - `expr.await` 经 poll 状态机轮询子 future，支持挂起（Poll::Pending）与恢复
// - 跨 await 的 i64 变量与子 future 提升为结构体字段
// 输出与 async-fns.out 精确对比

// 无 await 的 async fn（单个尾段）
async fn get_value(x: i64) -> i64 {
    x * 2
}

// async fn 内 .await 另一个 async fn（嵌套）
async fn nested(x: i64) -> i64 {
    let v = get_value(x).await;
    v + 1
}

// 多 await 顺序执行 + 跨段变量
async fn chain(x: i64) -> i64 {
    let a = get_value(x).await;
    let b = get_value(a).await;
    a + b
}

// 语句形式 await + return await
async fn early(x: i64) -> i64 {
    get_value(1).await;
    return get_value(x).await;
}

// 手动 Future：首次 Pending、二次 Ready（验证恢复轮询）
struct H { state: i64, x: i64 }

impl Future for H {
    type Output = i64;
    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output> {
        if self.state == 0 {
            self.state = 1;
            Poll::Pending
        } else {
            Poll::Ready(self.x)
        }
    }
}

fn mk_h(x: i64) -> H {
    H { state: 0, x: x }
}

// 跨 await 的 Pending 恢复
async fn pending_chain() -> i64 {
    let f1: H = mk_h(99);
    let a = f1.await;
    let f2: H = mk_h(100);
    let b = f2.await;
    a + b
}

fn main() {
    // block_on 状态机轮询
    let mut fut1 = get_value(21);
    println(block_on(&mut fut1));        // 42
    // 嵌套 await
    let mut fut2 = nested(10);
    println(block_on(&mut fut2));        // 21
    // 多 await 链
    let mut fut3 = chain(3);
    println(block_on(&mut fut3));        // 6 + 12 = 18
    // return await
    let mut fut4 = early(20);
    println(block_on(&mut fut4));        // 40
    // Pending 恢复
    let mut fut5 = pending_chain();
    println(block_on(&mut fut5));        // 199
}
