// 期望编译失败：结构体字段类型不匹配（SH-P2-6 L2 多位置标注回指字段声明处）
// expect: expects `i64`, found `string`
// expect: = note: 字段 `x` 类型 `i64` 声明于此
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let p = Point { x: "hello", y: 2 };
}
