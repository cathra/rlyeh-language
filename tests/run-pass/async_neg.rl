// W2：if+await 控制流 await 状态机 desugar（poll 死循环已于 2026-08-29 验收修复，解除 skip）
async fn get_value(x: i64) -> i64 {
    x * 2
}

async fn ifneg(x: i64) -> i64 {
    if x > 0 {
        return get_value(100).await;
    }
    return get_value(-x).await;
}

fn main() {
    let mut f1 = ifneg(3);       // 应走 then：get_value(100)=200
    println(block_on(&mut f1));
    let mut f2 = ifneg(-3);      // 应走 else：get_value(3)=6
    println(block_on(&mut f2));
}
