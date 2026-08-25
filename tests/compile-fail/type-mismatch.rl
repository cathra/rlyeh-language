// 期望编译失败：类型不匹配（i64 注解 vs 字符串字面量）
// expect: expected `i64`, found `string`
fn main() {
    let x: i64 = "hello";
    println(x);
}
