struct Inner {
    x: i64,
    y: i64,
}
impl Inner {
    fn sum(&self) -> i64 { self.x + self.y }
}

fn main() {
    let w = Inner { x: 3, y: 4 };
    let r = &w;
    println(r.x);          // 期望 3
    println(r.sum());      // 期望 7
}
