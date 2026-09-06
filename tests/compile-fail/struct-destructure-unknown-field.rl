// 结构体解构引用不存在的字段须报错（而非静默误绑定）。
// expect: has no field
struct Point { x: i64, y: i64 }
fn main() {
    let p = Point { x: 1, y: 2 };
    let Point { z } = p;
    println(z);
}
