// 期望编译失败：函数实参类型不匹配（SH-P2-6 L2 多位置标注回指形参声明处）
// expect: expects `i64`, found `String`
// expect: = note: 形参 #2 声明于此
fn add(a: i64, b: i64) -> i64 {
    a + b
}

fn main() {
    let _ = add(1, "two");
}
