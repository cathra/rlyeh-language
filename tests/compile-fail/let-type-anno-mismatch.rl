// expect: = note: 类型标注
// SH-P2-6 L2：`let x: T = e;` 类型标注不匹配时，`= note:` 次级标注应回指类型标注 `T` 处。
fn main() {
    let x: i64 = "hello";
}
