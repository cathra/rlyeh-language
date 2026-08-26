// 期望编译失败：泛型 async fn 作为子 future await 暂不支持（MVP，
// 独立 block_on 支持）。
// expect: 泛型 async fn `echo` 作为子 future await 暂不支持
async fn echo<T>(x: T) -> T {
    x
}

async fn wrapper(y: i64) -> i64 {
    let v = echo(y).await;
    v + 1
}

fn main() {
    let mut w = wrapper(41);
    println(block_on(&mut w));
}
