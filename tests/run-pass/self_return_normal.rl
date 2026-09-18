// 普通（非 dyn）方法按值返回 Self 聚合，定位 codegen 返回约定
protocol Copyable {
    fn make(&self) -> Self;
}
struct Pair { a: i64, b: i64 }
impl Pair: Copyable {
    fn make(&self) -> Self { Pair { a: self.a, b: self.b } }
}
fn main() {
    let p = Pair { a: 3, b: 4 };
    let c = p.make();
    println(c.a);
    println(c.b);
}
