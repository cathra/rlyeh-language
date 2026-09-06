// compile-fail：枚举变体结构式构造必须提供全部字段
// expect: missing field
enum Shape { Rect { w: i64, h: i64 } }
fn main() {
    let r = Shape::Rect { w: 1 };
    println(r);
}
