// `mut` 绑定修饰符（SH-P1-2 续，2026-09-06）：`let` 解构模式内的 `mut x` 令该绑定为可变。
// 覆盖元组元素 / 整体 mut 元组 / 结构体字段 / 枚举负载 / 嵌套元组 / 混合绑定。
enum Opt { None, Some(i64) }
struct Point { x: i64, y: i64 }

fn main() {
    // 1. 元组元素 `mut`
    let (mut a, b) = (1, 2);
    a = 10;
    println(a);                              // 10
    println(b);                              // 2

    // 2. 整体 `mut` 元组（两元素皆可变）
    let mut (x, y) = (3, 4);
    x = 30;
    y = 40;
    println(x);                              // 30
    println(y);                              // 40

    // 3. 结构体字段 `mut`
    let Point { mut x: px, y: py } = Point { x: 5, y: 6 };
    px = 50;
    println(px);                             // 50
    println(py);                             // 6

    // 4. 枚举负载 `mut`
    let Opt::Some(mut v) = Opt::Some(7);
    v = 70;
    println(v);                              // 70

    // 5. 嵌套元组：内层元素 `mut`
    let (mut (p, q), r) = ((1, 2), 3);
    p = 10;
    q = 20;
    println(p);                              // 10
    println(q);                              // 20
    println(r);                              // 3

    // 6. 混合：仅第二个元素 `mut`
    let (a2, mut b2) = (8, 9);
    b2 = 90;
    println(a2);                             // 8
    println(b2);                             // 90
}
