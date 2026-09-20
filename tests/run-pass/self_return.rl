// U4 `-> Self` 返回：protocol 方法 / static 方法 / inherent 方法签名返回 Self
protocol Clone {
    fn clone(&self) -> Self;
}

protocol Zero {
    fn zero() -> Self;
}

struct Point { x: i64, y: i64 }
impl Point: Clone {
    fn clone(&self) -> Self {
        Point { x: self.x, y: self.y }
    }
}
impl Point: Zero {
    fn zero() -> Self {
        Point { x: 0, y: 0 }
    }
}

// inherent 方法同样支持 `-> Self`
impl Point {
    fn doubled(&self) -> Self {
        Point { x: self.x * 2, y: self.y * 2 }
    }
    fn moved(dx: i64, dy: i64) -> Self {
        Point { x: dx, y: dy }
    }
}

fn main() {
    let p = Point { x: 3, y: 4 };
    let c = p.clone();        // protocol 方法返回 Self → Point
    println(c.x + c.y);       // 7
    let d = p.doubled();      // inherent 方法返回 Self
    println(d.x + d.y);       // 14
    let z = Point::zero();    // static 方法返回 Self
    println(z.x + z.y);       // 0
    let m = Point::moved(10, 20);
    println(m.x * m.y);       // 200
}
