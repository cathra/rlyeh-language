// 期望编译失败：已注解（已固化）捕获闭包值作函数返回值。
// 捕获聚合对象不跨函数边界（MVP 限制），与返回类型 fn 不匹配。
// expect: expected `fn(i64) -> i64`, found `closure(
fn make() -> fn(i64) -> i64 {
    let base = 10;
    let f = |x: i64| x + base;
    f
}
fn main() {
    let m = make();
    println(m(1));
}
