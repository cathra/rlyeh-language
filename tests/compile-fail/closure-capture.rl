// 期望编译失败：闭包捕获外部变量（H3 规划，H2 仅支持无捕获）
// expect: 闭包捕获外部变量
fn main() {
    let base = 10;
    let add_base: fn(i64) -> i64 = |x| x + base;
    let r = add_base(1);
    println(r);
}
