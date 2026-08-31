// 对应 docs/guide/05-aggregates-generics.md —— 聚合类型与泛型
// 运行：rlyeh run 05-aggregates-generics.rl
enum Shape {
    Circle(f64),
    Rect { w: f64, h: f64 },
    Triangle(f64, f64),         // 练习 1：新增变体，match 必须覆盖
}

fn area(s: Shape) -> f64 {
    match s {
        Shape::Circle(r) => 3.14 * r * r,
        Shape::Rect { w, h } => w * h,
        Shape::Triangle(a, b) => a * b / 2.0,
    }
}

fn main() {
    println(area(Shape::Circle(2.0)));       // 12.56
    println(area(Shape::Rect { w: 3.0, h: 4.0 }));  // 12
    println(area(Shape::Triangle(3.0, 4.0)));       // 6
}
