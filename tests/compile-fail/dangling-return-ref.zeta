// G1 严格借用检查：悬垂引用——直接返回局部变量的引用（E0597）。
// expect: `x` does not live long enough
fn f() -> &i64 {
    let x = 1;
    &x
}
fn main() {
    println(0);
}
