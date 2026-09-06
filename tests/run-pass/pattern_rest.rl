// 剩余模式 `..` 末位吸收（SH-P1-2 收尾，2026-09-06）：
// 元组 / 结构体 / 枚举解构在 `let` 与 `match` 位置均支持末位 `..`，吸收其余元素 / 字段。

enum Opt { None, Some(i64) }

struct Point { x: i64, y: i64, z: i64 }

fn main() {
    // 1. 元组末位 `..`
    let (a, ..) = (1, 2, 3, 4);
    println(a);                              // 1

    // 2. 元组前段绑定 + 末位 `..`
    let (b, c, ..) = (5, 6, 7, 8);
    println(b);                              // 5
    println(c);                              // 6

    // 3. 结构体末位 `..`
    let Point { x, .. } = Point { x: 10, y: 20, z: 30 };
    println(x);                              // 10

    // 4. 结构体前段绑定 + 末位 `..`
    let Point { x: px, y: py, .. } = Point { x: 11, y: 22, z: 33 };
    println(px);                             // 11
    println(py);                             // 22

    // 5. 枚举末位 `..`
    let Opt::Some(v, ..) = Opt::Some(42);
    println(v);                              // 42

    // 6. match 元组 `..`
    let t = (100, 200, 300);
    match t {
        (h, ..) => { println(h); }           // 100
    }

    // 7. match 结构体 `..`
    let p = Point { x: 7, y: 8, z: 9 };
    match p {
        Point { x: rx, .. } => { println(rx); }   // 7
    }

    // 8. match 枚举 `..`
    let o = Opt::Some(99);
    match o {
        Opt::Some(rv, ..) => { println(rv); }     // 99
        Opt::None => { println(-1); }
    }

    // 9. 嵌套：元组元素内枚举 `..`
    let (Opt::Some(n, ..), m) = (Opt::Some(123), 456);
    println(n);                              // 123
    println(m);                              // 456
}
