// 算术 + 控制流 + bool 打印：输出与 arith.out 精确对比
fn add(a: i64, b: i64) -> i64 {
    a + b
}

fn main() {
    let x = 6;
    let y = 7;
    let sum = add(x, y);
    println(sum);
    println("6 * 7 = ");
    println(x * y);
    println();
    if sum > 10 {
        println("sum is big");
    };
    println(true);
    println(false);
}
