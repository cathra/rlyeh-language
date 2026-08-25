// K1 `?` 错误传播：Option 上下文 desugar 为 match + return
fn try_div(x: i64, y: i64) -> Option<i64> {
    if y == 0 {
        return None;
    }
    Some(x / y)
}

fn chain(a: i64, b: i64, c: i64) -> Option<i64> {
    let q = try_div(a, b)?;   // 解包，失败早返回 None
    let r = try_div(q, c)?;   // 第二次解包
    Some(r + 1)
}

// 表达式中间使用 ?（两个 ? 嵌套于一个表达式）
fn sum_all(a: i64, b: i64, c: i64) -> Option<i64> {
    Some(try_div(a, b)? + try_div(c, 2)?)
}

fn main() {
    // 成功链：try_div(10,2)=5 → try_div(5,3)=1 → Some(2)
    match chain(10, 2, 3) {
        Some(v) => println(v),
        None => println(-1),
    }
    // 中途除零 → None
    match chain(10, 0, 3) {
        Some(v) => println(v),
        None => println(-2),
    }
    // 尾部除零 → None
    match chain(10, 2, 0) {
        Some(v) => println(v),
        None => println(-3),
    }
    // 表达式中间两个 ?：8/4=2 + 6/2=3 = 5
    match sum_all(8, 4, 6) {
        Some(v) => println(v),
        None => println(-4),
    }
    // 第一个 ? 失败 → None
    match sum_all(8, 0, 6) {
        Some(v) => println(v),
        None => println(-5),
    }
}
