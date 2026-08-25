// G3 裸指针 `*const T` / `*mut T`（MVP）
struct Point {
    x: i64,
    y: i64,
}

fn incr_ptr(p: *const i64) -> i64 {
    *p + 1
}

fn main() {
    // 基本：`&x` 隐式转为 `*const T`，`*p` 读取
    let x = 42;
    let p: *const i64 = &x;
    println(*p);                    // 42

    // *mut 写入后经原变量可见
    let mut y = 7;
    let q: *mut i64 = &mut y;
    *q = 100;
    println(y);                     // 100
    println(*q);                    // 100

    // `*mut T` 降级为 `*const T`
    let r: *const i64 = q;
    println(*r);                    // 100

    // 函数参数传 `&x`
    let z = incr_ptr(&x);
    println(z);                     // 43

    // 聚合裸指针：`(*p).field`（需显式解引用，不自动剥层）
    let pt = Point { x: 10, y: 20 };
    let pp: *const Point = &pt;
    println((*pp).x + (*pp).y);     // 30

    // 返回值：函数返回 *const T
    let sp = make_ptr(&x);
    println(*sp);                   // 42
}

fn make_ptr(v: &i64) -> *const i64 {
    v
}
