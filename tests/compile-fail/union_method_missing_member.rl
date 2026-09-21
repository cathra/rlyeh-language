// 联合方法分发（开放问题①）负例：方法 `sound` 仅存在于 `Dog`，
// 标量成员 `i64` 无 `sound`，故不得在联合上调用（应报 UnionMethodNotCommon）。

struct Dog { v: i64 }
impl Dog {
    fn sound(&self) -> i64 { self.v + 100 }
}

fn main() {
    let d = Dog { v: 2 };
    let u: i64 | Dog = d;
    println(u.sound());
}
