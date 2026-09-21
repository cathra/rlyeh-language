// 联合方法分发（type-union §9 开放问题①）：接收者为类型联合时，
// 若方法对所有成员都存在且签名一致，不经显式 `match` 直接经联合调用；
// 编译器 desugar 为按成员类型臂分发的 `match`（复用现有 union type-arm 收窄）。

struct Cat { v: i64 }
struct Dog { v: i64 }
struct Bird { v: i64 }

impl Cat {
    fn sound(&self) -> i64 { self.v }
}
impl Dog {
    fn sound(&self) -> i64 { self.v + 100 }
}
impl Bird {
    fn sound(&self) -> i64 { self.v + 200 }
}

fn main() {
    let c = Cat { v: 1 };
    let u1: Cat | Dog = c;
    println(u1.sound());        // 1

    let d = Dog { v: 2 };
    let u2: Cat | Dog = d;
    println(u2.sound());        // 102

    // 同一联合变量、带实参的方法同样正确分发
    let u3: Cat | Dog = c;
    println(u3.sound());        // 1

    // 三个成员的方法分发
    let b = Bird { v: 3 };
    let u4: Cat | Dog | Bird = b;
    println(u4.sound());        // 203
}
