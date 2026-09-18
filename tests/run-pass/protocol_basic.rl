// 协议语法（见 docs/rfc/protocol-syntax.md）：
// - `protocol P { .. }` 声明协议；
// - `impl Sq: Area { ... }` 协议一致性、`impl Sq { ... }` 固有实现；
// - 协议默认方法经空 `impl` 块回退。

protocol Area {
    fn area(&self) -> i64;
}

protocol Greeter {
    fn hello(&self) -> i64 { 100 }   // 协议默认方法
}

struct Sq {
    s: i64,
}

impl Sq: Area {
    fn area(&self) -> i64 { self.s * self.s }
}

impl Sq: Greeter {
    // 不实现 hello → 回退协议默认
}

impl Sq {
    fn new(s: i64) -> Sq { Sq { s: s } }
}

fn main() {
    let q = Sq::new(3);
    println(q.area());             // 9
    println(q.hello());            // 100
    println(q.area() + q.hello()); // 109
}
