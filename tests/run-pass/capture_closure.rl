// H3 捕获闭包（IIFE：`(|x| body)(args)` 立即调用）
fn main() {
    // 单捕获：外层变量 a
    let a = 10;
    let r1 = (|x| x + a)(1);
    println(r1);                    // 11

    // 多捕获：a 与 b
    let b = 5;
    let r2 = (|x, y| x * a + y - b)(2, 3);
    println(r2);                    // 18

    // 捕获字符串变量（String 拼接）
    let name = String::from("rlyeh");
    let r3 = (|s| s + name)(String::from("hi "));
    println(r3);                    // hi rlyeh

    // 无捕获闭包仍走 H2 函数指针路径（回归）
    let r4 = (|x| x * 2)(21);
    println(r4);                    // 42

    // 参数遮蔽捕获（Rust 语义：参数同名优先）
    let v = 100;
    let r5 = (|v| v + 1)(7);
    println(r5);                    // 8

    // move 关键字：MVP 忽略所有权差异，仍按值捕获
    let c = 3;
    let r6 = (move |x| x + c)(9);
    println(r6);                    // 12

    // 捕获数组变量做索引
    let base = [1, 2, 3];
    let r7 = (|i| base[i] * 2)(1);
    println(r7);                    // 4

    // 捕获多个类型混合 + 嵌套表达式
    let factor = 3;
    let mut total = 0;
    for i in 0..<3 {
        total += (|j| j * factor)(i);
    }
    println(total);                 // 0 + 3 + 6 = 9
}
