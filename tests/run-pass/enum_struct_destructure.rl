// SH-P1-2 续（2026-09-06）：枚举变体**结构式负载**模式 `Shape::Rect { w, h }`
// 在 `let` / `match` / `if let` 位置均已支持（parser 此前仅接受 `(` 元组负载，
// `{` 结构式负载报语法错误）。命名字段经 typecheck 按变体声明顺序重排为位置
// 子模式后复用既有枚举收窄逻辑（与 `Enum::Variant(x)` 同构）；缺失字段补 `_`
// （部分解构），未知名报 `has no field`。

enum Shape { Circle(i64), Rect { w: i64, h: i64 } }

fn main() {
    let r = Shape::Rect { w: 3, h: 4 };

    // 1. match 枚举结构式负载：顶层（含穷尽兜底）
    match r {
        Shape::Rect { w, h } => { println(w); println(h); },   // 3, 4
        Shape::Circle(_) => {},
    }

    // 2. match 枚举结构式负载 + 字面量子模式 + 兜底
    match r {
        Shape::Rect { w: 0, h } => { println(h); },
        Shape::Rect { w, h } => { println(w + h); },            // 7
        Shape::Circle(_) => {},
    }

    // 3. let 位置枚举结构式负载（含字段重命名）
    let Shape::Rect { w: rw, h: rh } = r;
    println(rw + rh);                                          // 7

    // 4. if let 枚举结构式负载（desugar 成 match，自动继承）
    if let Shape::Rect { w, h } = r {
        println(w * h);                                        // 12
    }

    // 5. 嵌套：元组元素为枚举结构式负载
    let t = (Shape::Rect { w: 2, h: 5 }, 10);
    match t {
        (Shape::Rect { w, h }, n) => { println(w + h + n); },  // 17
    }

    // 6. 枚举结构式负载 + 枚举元组负载混用（区分两种语法）
    let c = Shape::Circle(9);
    match c {
        Shape::Circle(r) => { println(r); },                    // 9
        Shape::Rect { w, h } => { println(w + h); },
    }

    // 7. 部分解构（省略字段补 Wildcard）
    match r {
        Shape::Rect { w } => { println(w); },                   // 3
        Shape::Circle(_) => {},
    }
}
