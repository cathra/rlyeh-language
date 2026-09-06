// SH-P1-2（2026-09-06）：match 位置元组 / 结构体解构
// 让 `match` 臂支持 元组 / 结构体 模式（与 `let` 位置同构）——按位置 / 按名
// `FieldGet` 后递归 `check_pattern`（嵌套元组 / 结构体 / 枚举 / 字面量子模式
// 均经递归处理）。`if let` / `while let` 经 parser desugar 成 `match` 自动继承。
// 注：枚举变体模式仅支持 `(` 元组负载（结构式 `{..}` 负载为 parser 已知缺口，
// `let` 位置同此限制），故本用例枚举只用元组负载，嵌套解构以 元组 / 结构体 为主。

struct Point { x: i64, y: i64 }
struct Pair { p: (i64, i64) }
enum Shape { Pair(i64, i64), Single(i64) }

fn main() {
    // 1. match 顶层元组模式（不可反驳，直接兜底）
    let t = (10, 20);
    match t {
        (a, b) => { println(a); println(b); },    // 10, 20
    }

    // 2. match 元组 + 字面量子模式 + 兜底
    let u = (0, 5);
    match u {
        (0, x) => { println(x); },                // 5
        (a, b) => { println(a + b); },
    }

    // 3. match 结构体模式（顶层）
    let p = Point { x: 3, y: 4 };
    match p {
        Point { x, y } => { println(x); println(y); },  // 3, 4
    }

    // 4. match 结构体 + 字面量子模式 + 兜底
    match p {
        Point { x: 0, y } => { println(y); },
        Point { x, y } => { println(x + y); },     // 7
    }

    // 5. 枚举变体（元组 payload）解构
    let s1 = Shape::Pair(2, 3);
    match s1 {
        Shape::Pair(a, b) => { println(a + b); },   // 5
        Shape::Single(n) => { println(n); },
    }

    // 6. 嵌套：元组模式内含枚举子模式
    let w = (Shape::Pair(1, 2), 5);
    match w {
        (Shape::Pair(a, b), n) => { println(a + b + n); },  // 8
    }

    // 7. 嵌套：match 结构体字段为元组 `field: (a, b)`
    let pr = Pair { p: (7, 8) };
    match pr {
        Pair { p: (pa, pb) } => { println(pa + pb); },  // 15
    }

    // 8. if let 元组模式（desugar 成 match，自动继承）
    let v = (8, 9);
    if let (m, n) = v {
        println(m + n);                          // 17
    }

    // 9. if let 结构体模式
    let q = Point { x: 1, y: 2 };
    if let Point { x: qx, y: qy } = q {
        println(qx + qy);                        // 3
    }

    // 10. while let 枚举（元组 payload）内嵌，循环体内退出
    let mut opt = Option::Some((1, 2));
    while let Option::Some((x, y)) = opt {
        println(x + y);                          // 3
        opt = Option::None;
    }
}
