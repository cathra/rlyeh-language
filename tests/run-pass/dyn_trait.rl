// H4 `dyn Trait` trait 对象（MVP）：数据指针 + vtable 胖指针
protocol Shape {
    fn area(&self) -> f64;
    fn sides(&self) -> i64;
}

struct Circle {
    radius: f64,
}
impl Circle: Shape {
    fn area(&self) -> f64 {
        3.14 * self.radius * self.radius
    }
    fn sides(&self) -> i64 {
        0
    }
}

struct Rect {
    w: f64,
    h: f64,
}
impl Rect: Shape {
    fn area(&self) -> f64 {
        self.w * self.h
    }
    fn sides(&self) -> i64 {
        4
    }
}

fn main() {
    // `&T` → `dyn Trait` 强制转换：构造 vtable + 2 槽胖指针
    let c = Circle { radius: 2.0 };
    let r = Rect { w: 3.0, h: 4.0 };
    let d1: dyn Shape = &c;
    let d2: dyn Shape = &r;

    // vtable 间接调用：同一签名分派到不同 impl
    println(d1.area());     // 12.56
    println(d2.area());     // 12.0
    println(d1.sides());    // 0
    println(d2.sides());    // 4

    // 胖指针拷贝共享同一 vtable
    let d3 = d1;
    println(d3.area());     // 12.56

    // 带参数的方法
    let scale = scale_area(&c, 2.0);
    println(scale);         // 25.12
}

fn scale_area(s: &Circle, k: f64) -> f64 {
    let d: dyn Shape = s;
    d.area() * k
}
