// compile-fail：match 结构体模式字段名必须存在于被匹配结构体
// expect: has no field
struct Point { x: i64, y: i64 }
fn main() {
    let p = Point { x: 1, y: 2 };
    match p {
        Point { z } => { println(z); },
    }
}
