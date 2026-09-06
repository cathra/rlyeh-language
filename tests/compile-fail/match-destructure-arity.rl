// compile-fail：match 元组模式元数需与 scrutinee 元组一致
// expect: 3 元解构模式
fn main() {
    let t = (1, 2);
    match t {
        (a, b, c) => { println(a); },
    }
}
