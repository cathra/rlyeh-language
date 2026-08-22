// K4 追踪 GC：Gc<T> 函数参数 / 返回值 / 显式类型注解
struct Point {
    x: i64,
    y: i64,
}

fn identity(g: Gc<i64>) -> Gc<i64> {
    g
}

fn sum_points(a: Gc<Point>, b: Gc<Point>) -> i64 {
    a.x + b.x + a.y + b.y
}

fn main() {
    // 显式类型注解 + 函数返回值传递
    let g: Gc<i64> = gc_region { Gc::new(42) };
    let g2: Gc<i64> = identity(g);
    println(*g2); // 42

    // Gc<Point> 函数参数（字段剥层在函数内生效）
    let pa = gc_region { Gc::new(Point { x: 1, y: 2 }) };
    let pb = gc_region { Gc::new(Point { x: 10, y: 20 }) };
    println(sum_points(pa, pb)); // 33

    // 块内 Gc<T> 注解
    gc_region {
        let x: Gc<i64> = Gc::new(7);
        println(*x); // 7
    }
}
