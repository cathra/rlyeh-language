// 期望编译失败：有捕获的闭包值不能作 fn 实参（捕获对象不跨函数边界，MVP 限制）
// expect: found `closure(
fn apply(f: fn(i64) -> i64, x: i64) -> i64 { f(x) }
fn main() {
    let base = 10;
    let f = |x: i64| x + base;
    println(apply(f, 1));
}
