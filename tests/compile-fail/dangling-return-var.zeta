// G1 严格借用检查：悬垂引用——返回绑定局部变量引用的变量（E0597）。
// expect: `r` does not live long enough
fn f() -> &i64 {
    let x = 1;
    let r = &x;
    r
}
fn main() {
    println(0);
}
