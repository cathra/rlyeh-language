// 探针：递归 async fn（自递归 + 双函数依赖环）
async fn countdown(n: i64) -> i64 {
    if n == 0 {
        return 0;
    }
    let r = countdown(n - 1).await;
    r + 1
}

async fn ping(x: i64) -> i64 {
    if x <= 0 {
        return 100;
    }
    let r = pong(x - 1).await;
    r + 1
}
async fn pong(x: i64) -> i64 {
    let r = ping(x - 1).await;
    r + 2
}

fn main() {
    let mut f1 = countdown(3);
    println(block_on(&mut f1));
    let mut f2 = ping(2);
    println(block_on(&mut f2));
}
