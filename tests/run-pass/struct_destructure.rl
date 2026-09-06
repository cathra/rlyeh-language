// 结构体解构绑定 `let Point { x, y } = e;`（SH-P0-5 延伸，2026-09-06）：
// 元素模式支持标识符 / `_` / 嵌套 `Tuple` 字段 / 嵌套 `Struct` 字段；
// 字段按名取（`FieldGet` index = 字段定义序），与 `p.x` 同构；支持 `mut`。
struct Point {
    x: i64,
    y: i64,
}

struct Line {
    start: Point,
    end: Point,
    label: String,
}

struct Pair { p: (i64, i64) }

fn main() {
    let p = Point { x: 3, y: 4 };

    // 基本解构（按名绑定，顺序无关）
    let Point { x, y } = p;
    println(x);          // 3
    println(y);          // 4

    // 部分解构：仅列出需要的字段（不需 `..` 剩余模式）
    let Point { x: x2 } = p;
    println(x2);         // 3
    let Point { y: y2 } = p;
    println(y2);         // 4

    // 字段重命名 `field: new_name`
    let Point { x: renamed } = p;
    println(renamed);    // 3

    // 含堆分配字段（异构）
    let l = Line {
        start: Point { x: 1, y: 2 },
        end: Point { x: 3, y: 4 },
        label: String::from("diag"),
    };
    let Line { start: Point { x: sx, y: sy }, end: e, label } = l;
    println(sx);         // 1
    println(sy);         // 2
    println(e.x);        // 3
    println(e.y);        // 4
    println(label);      // diag

    // 嵌套元组字段 `field: (a, b)`
    let pr = Pair { p: (7, 8) };
    let Pair { p: (a, b) } = pr;
    println(a);          // 7
    println(b);          // 8

    // `mut` 解构：字段可重新赋值
    let mut Point { x: mx, y: my } = p;
    mx = 10;
    println(mx + my);    // 14

    // `&` 到结构体（剥一层引用）
    let Point { x: rx } = &p;
    println(rx);         // 3

    if x == 3 && y == 4 && x2 == 3 && y2 == 4 && renamed == 3
       && sx == 1 && sy == 2 && e.x == 3 && e.y == 4 && label == "diag"
       && a == 7 && b == 8 && mx == 10 && rx == 3 {
        println(1);      // 1
    } else {
        println(0);
    }
}
