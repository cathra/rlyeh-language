// 期望编译失败：结构体字段不存在
// expect: has no field
struct Point { x: i64, y: i64 }

fn main() {
    let p = Point { x: 1, y: 2 };
    println(p.z);
}
