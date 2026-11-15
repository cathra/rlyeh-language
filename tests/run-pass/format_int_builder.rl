// format! 性能改造（String builder + push_i64/push_bytes）语义回归：
// 零 / 负数 / 普通整数 / 字面量与整数混合。
fn main() {
    println(format!("{} {} {}", 0, -123, 456)); // 0 -123 456
    println(format!("key_{}", 42));             // key_42
    println(format!("{}", -9223372036854775807)); // i64::MIN + 1
}
