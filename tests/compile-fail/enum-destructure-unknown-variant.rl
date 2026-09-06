// 枚举解构引用不存在的变体须报错。
// expect: Option::Nope
fn main() {
    let Option::Nope(x) = Option::Some(1);
    println(x);
}
