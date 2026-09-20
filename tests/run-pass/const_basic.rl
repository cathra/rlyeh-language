// SH-P2-9 M1（const 编译期常量传播 + 算术求值）：模块级 const 在引用处内联，
// 含跨 const 引用（ANSWER 引用 BASE）与字面量算术（BASE + 2）。
const PI: f64 = 3.14159;
const BASE: i64 = 40;
fn main() -> i64 {
    println(PI);
    const ANSWER: i64 = BASE + 2;
    println(ANSWER);
    0
}
