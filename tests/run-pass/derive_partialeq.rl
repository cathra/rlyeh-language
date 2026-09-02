// SH-P1-2（0.2.0-C）：#[derive(PartialEq)] 自动合成 `impl PartialEq for Point`，
// 并通过 `==` / `!=` 落点 `a.eq(&b)`（comparison desugar）。
#[derive(PartialEq)]
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let a = Point { x: 1, y: 2 };
    let b = Point { x: 1, y: 2 };
    let c = Point { x: 3, y: 4 };
    println(if a == b { 1 } else { 0 });
    println(if a == c { 1 } else { 0 });
    println(if a != c { 1 } else { 0 });
}
