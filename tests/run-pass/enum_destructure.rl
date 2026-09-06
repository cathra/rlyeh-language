// 枚举解构绑定 `let Option::Some(x) = e;`（SH-P0-5 延伸，2026-09-06）：
// 与 match 位置收窄同构，仅绑定不运行时校验；支持嵌套元组 / 结构体 / 枚举子模式。

struct Point { x: i64, y: i64 }
struct Wrapper { v: Option<Point> }

fn main() {
    // 1. 基本枚举解构
    let Option::Some(v) = Option::Some(42);
    println(v);                              // 42

    // 2. 枚举 + 元组字段
    let Option::Some((a, b)) = Option::Some((1, 2));
    println(a);                              // 1
    println(b);                              // 2

    // 3. 枚举 + 结构体字段
    let Option::Some(Point { x, y }) = Option::Some(Point { x: 3, y: 4 });
    println(x);                              // 3
    println(y);                              // 4

    // 4. 元组内嵌枚举 `let (Option::Some(c), d) = e;`
    let (Option::Some(c), d) = (Option::Some(5), 6);
    println(c);                              // 5
    println(d);                              // 6

    // 5. 结构体字段内嵌枚举
    let w = Wrapper { v: Option::Some(Point { x: 7, y: 8 }) };
    let Wrapper { v: Option::Some(p) } = w;
    println(p.x);                            // 7
    println(p.y);                            // 8

    // 6. 嵌套枚举 `let Option::Some(Option::Some(n)) = e;`
    let Option::Some(Option::Some(n)) = Option::Some(Option::Some(9));
    println(n);                              // 9

    // 7. 通配符跳过字段
    let Option::Some(_) = Option::Some(100);
    println(10);                             // 10
}
