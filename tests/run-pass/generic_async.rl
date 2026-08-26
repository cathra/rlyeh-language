// W6：泛型 async fn（参数/返回泛型 T，独立 block_on）
// - 无 await 泛型 echo<T>：返回参数 T（透传 type Output = T）
// - 泛型 async fn 作为子 future await 暂不支持（独立 block_on 支持，
//   见 compile-fail/generic_async_subfuture.rl）

async fn echo<T>(x: T) -> T {
    x
}

fn main() {
    // T=i64
    let mut f = echo(10);
    println(block_on(&mut f));        // 10
    // T=String
    let mut g = echo(String::from("hi"));
    println(block_on(&mut g));        // hi
    // T=f64
    let mut h = echo(3.5);
    println(block_on(&mut h));        // 3.5
}
