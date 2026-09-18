// PC-1：声明点一致性（`struct C: P` / `enum E: P`）+ 类型体内联成员。
// desugar 归一为既有 impl：`struct C: P { fields; members }` → `struct C { fields }` + `impl P for C { members }`。

protocol Area {
    fn area(&self) -> i64;
}

struct Sq: Area {
    s: i64,
    fn area(&self) -> i64 { self.s * self.s }
}

protocol Greeter {
    fn hello(&self) -> i64 { 100 }   // 默认方法
}

struct Bar: Greeter {
    n: i64,
    // 不实现 hello → 回退默认（空一致性 impl）
}

// 枚举声明点一致性 + 内联方法（带负载变体 → 非标量枚举）
enum Shape: Area {
    Circle(i64),
    Empty,
    fn area(&self) -> i64 {
        match self {
            Shape::Circle(r) => r * r,
            Shape::Empty => 0,
        }
    }
}

// 无协议的内联方法 → 固有 impl
struct Pt {
    x: i64,
    fn norm2(&self) -> i64 { self.x * self.x }
}

fn main() {
    let q = Sq { s: 3 };
    println(q.area());        // 9

    let b = Bar { n: 1 };
    println(b.hello());       // 100（默认方法回退）

    let c = Shape::Circle(5);
    println(c.area());        // 25

    let p = Pt { x: 4 };
    println(p.norm2());       // 16
}
