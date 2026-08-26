// W2：async fn 内 if 控制流 await 状态机 desugar
async fn get_value(x: i64) -> i64 {
    x * 2
}

// if 内 await 后 return（最简单）
async fn simple_if(x: i64) -> i64 {
    if x > 0 {
        return get_value(x).await;
    }
    0
}

// if 分支都 return await
async fn branch(x: i64) -> i64 {
    if x > 0 {
        return get_value(x).await;
    }
    return get_value(-x).await;
}

fn main() {
    let mut f1 = simple_if(5);
    println(block_on(&mut f1));     // 10
    let mut f2 = simple_if(-1);
    println(block_on(&mut f2));     // 0
    let mut f3 = branch(3);
    println(block_on(&mut f3));     // 6
    let mut f4 = branch(-3);
    println(block_on(&mut f4));     // 6
}
