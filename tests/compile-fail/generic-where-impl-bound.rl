// SH-P1-1 A4：impl 块级 `where` 子句的约束必须强制校验——impl 的类型参数由
// 接收者类型 unify 推导（`Pair<i64>` → T = i64），随后 `T: Speak` 不成立须报错。
// expect: does not implement protocol `Speak`
protocol Speak {
    fn speak(&self) -> i64;
}

struct Pair<T> { a: T }

protocol Wrap {
    fn wrap(&self) -> i64;
}

impl<T> Pair<T>: Wrap where T: Speak {
    fn wrap(&self) -> i64 { self.a.speak() }
}

fn main() {
    let p: Pair<i64> = Pair<i64> { a: 1 };
    println(p.wrap());
}
