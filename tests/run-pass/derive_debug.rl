// SH-P1-2（0.2.0-C）：#[derive(Debug)] 自动合成 `impl fmt::Debug for Point`，
// 经 `dbg!`（`{:?}` 语义）输出结构化表示。
#[derive(Debug)]
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let p = Point { x: 1, y: 2 };
    dbg!(p);
}
