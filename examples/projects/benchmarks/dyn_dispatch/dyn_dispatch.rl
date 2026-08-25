// 基准: dyn_dispatch —— 2000 万次 dyn Trait vtable 多态分派
// 测: 虚方法分派开销（Rlyeh 胖指针 + vtable 间接调用）
// 逻辑: 循环交替调用 Tri/Quad 的 dyn 对象方法 sides()（读字段）。
//       输出 = 10,000,000 * (3 + 4) = 70,000,000
trait Shape {
    fn sides(&self) -> i64;
}

struct Tri {
    n: i64,
}

impl Shape for Tri {
    fn sides(&self) -> i64 {
        self.n + 3
    }
}

struct Quad {
    n: i64,
}

impl Shape for Quad {
    fn sides(&self) -> i64 {
        self.n + 4
    }
}

fn main() {
    let t = Tri { n: 0 };
    let q = Quad { n: 0 };
    let d1: dyn Shape = &t;
    let d2: dyn Shape = &q;
    let mut sum = 0;
    let mut i = 0;
    while i < 10000000 {
        sum = sum + d1.sides();
        sum = sum + d2.sides();
        i = i + 1;
    }
    println(sum);
}
