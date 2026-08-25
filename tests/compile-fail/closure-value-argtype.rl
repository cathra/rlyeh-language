// 期望编译失败：闭包值调用实参类型与参数注解不匹配
// expect: expects `i64`, found `String`
fn main() {
    let f = |x: i64| x + 1;
    println(f("str"));
}
