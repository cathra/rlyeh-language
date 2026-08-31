// U3 核心项（2026-08-30）：标量枚举 → 整数单向兼容。反向（整数 → 枚举）
// 必须禁止，否则会构造出无对应判别式的非法枚举值。
// expect: expected `Color`, found `i64`
enum Color { Red, Green, Blue }

fn main() {
    let c: Color = 1;
    println(c);
}
