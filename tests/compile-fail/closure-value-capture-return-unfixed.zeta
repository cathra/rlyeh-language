// 期望编译失败：未固化（非注解）捕获闭包值作函数返回值。
// 按 fn 返回签名固化的过程中发现闭包捕获外部变量——捕获对象不跨函数边界（MVP 限制）。
// expect: 捕获闭包值不能经 fn 签名传递
fn make() -> fn(i64) -> i64 {
    let base = 10;
    let f = |x| x + base;
    f
}
fn main() {
    let m = make();
    println(m(1));
}
