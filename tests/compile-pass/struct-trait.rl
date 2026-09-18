// struct + impl + trait：编译必须成功
struct Point { x: i64, y: i64 }

protocol Area {
    fn area(&self) -> i64;
}

impl Point: Area {
    fn area(&self) -> i64 {
        self.x * self.y
    }
}

fn main() {
    let p = Point { x: 3, y: 4 };
    println(p.area());
}
