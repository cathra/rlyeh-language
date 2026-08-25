// struct + impl + trait：编译必须成功
struct Point { x: i64, y: i64 }

trait Area {
    fn area(&self) -> i64;
}

impl Area for Point {
    fn area(&self) -> i64 {
        self.x * self.y
    }
}

fn main() {
    let p = Point { x: 3, y: 4 };
    println(p.area());
}
