// U8：泛型结构体构造（Foo<i64> { .. }）+ 泛型 trait（trait Wrap<T> + impl<T> Wrap<T> for X）
struct Pair<T> { a: T }
impl<T> Pair<T> {
    fn get(&self) -> T {
        self.a
    }
    fn map<U>(&self, x: U) -> U {
        x
    }
}

protocol Wrap<T> {
    fn wrap(&self) -> T;
}
impl<T> Pair<T>: Wrap<T> {
    fn wrap(&self) -> T {
        self.a
    }
}

fn main() {
    // 泛型结构体构造（带类型实参）
    let p: Pair<i64> = Pair<i64> { a: 100 };
    println(p.get());        // 100（泛型 impl 方法）
    println(p.map(7));       // 7（方法泛型 U=i64）
    println(p.map("hi"));    // hi（方法泛型 U=String）
    println(p.wrap());       // 100（泛型 trait impl）
    // 字符串类型参数的泛型结构体构造
    let s: Pair<String> = Pair<String> { a: String::from("yo") };
    println(s.get());        // yo
    println(s.wrap());       // yo
}
