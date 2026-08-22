// H2 无捕获闭包：编译通过（不运行）
fn each3(f: fn(i64, i64, i64) -> i64, a: i64, b: i64, c: i64) -> i64 {
    f(a, b, c)
}

fn main() {
    // 三参数闭包 + fn 注解绑定
    let add3: fn(i64, i64, i64) -> i64 = |a, b, c| a + b + c;
    let r1 = add3(1, 2, 3);

    // 闭包作实参（多个不同闭包调用同一函数）
    let s = each3(|a, b, c| a * 2 + b * 2 + c * 2, 1, 2, 3);
    let t = each3(|a, b, c| a - b + c, 9, 2, 1);

    // 闭包返回布尔（if 表达式）
    let is_gt: fn(i64, i64) -> bool = |a, b| a > b;
    let b1 = is_gt(3, 1);

    // 闭包作为函数值再次传递（函数指针实参）
    let twice: fn(fn(i64) -> i64, i64) -> i64 = |f, x| f(f(x));
    let add2: fn(i64) -> i64 = |x| x + 2;
    let r2 = twice(add2, 5);
}
