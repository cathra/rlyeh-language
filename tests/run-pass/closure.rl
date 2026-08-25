// H2 无捕获闭包：`|x, y| expr` desugar 为匿名函数 + 函数指针，零运行时开销
fn apply(f: fn(i64, i64) -> i64, x: i64, y: i64) -> i64 {
    f(x, y)
}

fn max3(f: fn(i64, i64, i64) -> i64, a: i64, b: i64, c: i64) -> i64 {
    f(a, b, c)
}

fn main() {
    // 闭包作 fn 形参实参（预期签名驱动参数类型推断）
    let r1 = apply(|a, b| a + b, 10, 20);    // 30
    let r2 = apply(|a, b| a * b, 3, 4);      // 12
    let r3 = apply(|a, b| a - b, 20, 5);     // 15

    // 闭包 + fn 类型注解绑定，再经函数值间接调用
    let inc: fn(i64) -> i64 = |x| x + 1;
    let r4 = inc(41);                        // 42

    // 闭包体内更复杂表达式（算术混合）
    let r5 = apply(|a, b| a * 2 + b * 3, 1, 2);  // 8

    // 三参数闭包 + 多分支
    let r6 = max3(|a, b, c| a + b + c, 1, 2, 3);   // 6
    let r7 = max3(|a, b, c| a * b + c, 2, 3, 4);   // 10

    println(r1);
    println(r2);
    println(r3);
    println(r4);
    println(r5);
    println(r6);
    println(r7);
}
