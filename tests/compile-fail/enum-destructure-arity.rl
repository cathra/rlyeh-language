// 枚举解构元数不符须报错（而非静默误绑定）。
// expect: expects 1 arguments, found 2
fn main() {
    let Option::Some(a, b) = Option::Some(1);
    println(a);
}
