// U3 泛型约束（protocol bound）：声明 + 调用点校验（宽松：不推导）
protocol HasArea {
    fn area(&self) -> f64;
}

protocol Named {
    fn name(&self) -> String;
}

struct Point { x: i64, y: i64 }
impl Point: HasArea {
    fn area(&self) -> f64 { 0.0 }
}
impl Point: Named {
    fn name(&self) -> String { String::from("Point") }
}

struct Circle { r: f64 }
impl Circle: HasArea {
    fn area(&self) -> f64 { 3.14 * self.r * self.r }
}
impl Circle: Named {
    fn name(&self) -> String { String::from("Circle") }
}

// 单 bound：T 必须实现 HasArea（函数体内可调用 bound 中声明的方法）
fn scale<T: HasArea>(t: T, k: f64) -> f64 {
    t.area() * k
}

// 多 bound：T 必须同时实现 Named 与 HasArea
fn describe<T: Named + HasArea>(t: T) -> String {
    t.name()
}

// 混合：T 有 bound、U 无 bound
fn first_area<T: HasArea, U>(t: T, u: U) -> f64 {
    t.area()
}

// 泛型 bound 函数作为模板被多次实例化（单态化）
fn main() {
    let p = Point { x: 1, y: 2 };
    let c = Circle { r: 2.0 };
    println(scale(p, 2.0));    // 0.0
    println(scale(c, 2.0));    // 12.56 * 2 = 25.12
    println(describe(p));      // Point
    println(describe(c));      // Circle
    println(first_area(p, 5)); // 0.0
}
