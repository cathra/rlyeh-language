// 期望编译失败：泛型实参不满足 protocol bound（i64 未实现 HasArea）
// expect: does not implement protocol `HasArea`
protocol HasArea {
    fn area(&self) -> f64;
}

struct Point { x: i64, y: i64 }
impl Point: HasArea {
    fn area(&self) -> f64 { 0.0 }
}

fn scale<T: HasArea>(t: T, k: f64) -> f64 {
    t.area() * k
}

fn main() {
    let p = Point { x: 1, y: 2 };
    println(scale(p, 2.0));
    println(scale(5, 2.0));
}
