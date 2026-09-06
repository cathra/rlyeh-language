// compile-fail：枚举结构式负载模式字段名须存在于变体
// expect: has no field
enum Shape { Rect { w: i64, h: i64 } }
fn main() {
    let r = Shape::Rect { w: 1, h: 2 };
    match r {
        Shape::Rect { z } => { println(z); },
    }
}
