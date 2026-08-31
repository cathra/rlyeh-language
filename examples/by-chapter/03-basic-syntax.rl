// 对应 docs/guide/03-basic-syntax.md —— 基础语法
// 运行：rlyeh run 03-basic-syntax.rl
fn add(a: i64, b: i64) -> i64 {
    a + b                       // 末表达式为返回值（隐式 return）
}

// H2 无捕获闭包：闭包作 fn 形参实参
fn apply(f: fn(i64, i64) -> i64, x: i64, y: i64) -> i64 {
    f(x, y)
}

fn main() {
    // 变量遮蔽（块内 let 覆盖外层）
    let x = 10;
    {
        let x = 20;
        println(x);             // 20
    };
    println(x);                 // 10

    println(add(6, 7));         // 13

    // 无捕获闭包
    let r = apply(|a, b| a * b, 6, 7);
    println(r);                 // 42
}
