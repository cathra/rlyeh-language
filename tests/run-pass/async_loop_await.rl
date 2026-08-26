// W2：async fn 内 while/for 控制流 await 状态机 desugar
async fn get_value(x: i64) -> i64 {
    x * 2
}

// while 内 await（累加）
async fn while_sum(n: i64) -> i64 {
    let mut sum = 0;
    let mut i = 0;
    while i < n {
        sum = get_value(i).await;
        i = i + 1;
    }
    sum
}

// for 区间内 await
async fn for_sum(n: i64) -> i64 {
    let mut total = 0;
    for i in 0..<n {
        let v = get_value(i).await;
        total = total + v;
    }
    total
}

fn main() {
    let mut f1 = while_sum(3);   // i=0:0, i=1:2, i=2:4 → 最后一次 sum=4
    println(block_on(&mut f1));
    let mut f2 = while_sum(1);   // i=0:0 → sum=0
    println(block_on(&mut f2));
    let mut f3 = for_sum(3);     // 0+2+4=6
    println(block_on(&mut f3));
    let mut f4 = for_sum(4);     // 0+2+4+6=12
    println(block_on(&mut f4));
}
