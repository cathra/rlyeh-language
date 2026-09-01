// 普通函数按值返回聚合（无 trait / 无 Self），判定聚合按值返回是否通用可用
struct Pair { a: i64, b: i64 }
fn make_pair(x: i64) -> Pair { Pair { a: x, b: x + 1 } }
fn main() {
    let p = make_pair(3);
    println(p.a);
    println(p.b);
}
