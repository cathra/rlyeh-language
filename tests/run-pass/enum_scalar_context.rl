// U3 核心项（2026-08-30）：受限标量枚举紧凑为单标量存储（值即 tag），
// 可在整数上下文使用：数组索引 / 赋值整数 / 整数比较 / 位运算 / match 收窄。

enum Color { Red, Green, Blue }

fn main() {
    let c = Color::Green;
    let arr = [10, 20, 30];
    println(arr[c]);            // 20：标量枚举作数组索引（tag=1 → arr[1]）
    let n: i64 = c;
    println(n);                 // 1：赋值给整数（单向兼容，读取 tag）
    if c == Color::Green {
        println(1);             // 1：整数比较（tag 相等）
    }
    let mask = Color::Red | Color::Blue;
    println(mask);              // 2：位运算（0 | 2，结果按整数）
    // match 收窄仍可用（直接整数比较 scrutin == tag）
    match c {
        Color::Red => println("Red"),
        Color::Green => println("Green"),
        Color::Blue => println("Blue"),
    }
}
