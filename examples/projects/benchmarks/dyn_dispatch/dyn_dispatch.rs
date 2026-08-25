// 基准: dyn_dispatch —— 2000 万次多态分派（trait 对象 vtable）
// 与 dyn_dispatch.rl 逻辑严格一致。输出 = 70000000
trait Shape {
    fn sides(&self) -> i64;
}

struct Tri {
    n: i64,
}
impl Shape for Tri {
    fn sides(&self) -> i64 { self.n + 3 }
}

struct Quad {
    n: i64,
}
impl Shape for Quad {
    fn sides(&self) -> i64 { self.n + 4 }
}

fn main() {
    let t = Tri { n: 0 };
    let q = Quad { n: 0 };
    let d1: &dyn Shape = &t;
    let d2: &dyn Shape = &q;
    let mut sum: i64 = 0;
    for _ in 0..10_000_000 {
        sum += d1.sides();
        sum += d2.sides();
    }
    println!("{}", sum);
}
