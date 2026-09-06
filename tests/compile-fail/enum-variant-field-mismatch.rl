// expect: = note: 字段 `f0` 类型 `i64` 声明于此
// SH-P2-6 L2：枚举变体字段构造类型不匹配时，`= note:` 次级标注应回指该字段的
// 声明处（变体 `Red(i64)` 处）。
enum Color { Red(i64), Green(i64) }
fn main() {
    let c: Color = Color::Red("hello");
}
