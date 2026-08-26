async fn g(x: i64) -> i64 {
    x * 2
}

async fn wc(n: i64) -> i64 {
    let mut c = 0;
    while c < n {
        c = c + 1;
        let v = g(c).await;
        v;
    }
    c
}

fn main() {
    let mut f1 = wc(3);
    println(block_on(&mut f1));     // c 到 3，期望 3
}
