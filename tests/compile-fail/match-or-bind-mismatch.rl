// SH-P0-7 P-M3：或模式各备选必须绑定**同名同序**的变量集（与 Rust 一致）。
// `Some(x) | Some(y)` 这类「绑定数量相同但名字不同」必须显式报错，
// 否则命中的备选不同会导致变量未定义。
// expect: 或模式各备选绑定不一致
enum Shape { Circle(i64), Square(i64) }

fn main() {
    let s = Shape::Circle(3);
    match s {
        Shape::Circle(x) | Shape::Square(y) => println(x),
        _ => println(0),
    }
}
