// 期望编译失败：半注解闭包值绑定，注解类型与调用点实参推断类型冲突
// （x 注解 i64，实参为 String）
// expect: expects `i64`, found `String`
fn main() {
    let f = |x: i64, y| x + y;
    println(f("s", 2));
}
