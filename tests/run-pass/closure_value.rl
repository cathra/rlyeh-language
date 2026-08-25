// H5 闭包值对象：`let f = |x: i64| body;` 绑定后 `f(args)` 调用。
// 按值捕获 desugar 为聚合对象（每捕获一槽）+ 闭包匿名函数。
fn main() {
    // 基本：绑定 + 调用
    let f1 = |x: i64| x + 1;
    println(f1(41));                // 42

    // 多参数
    let f2 = |a: i64, b: i64| a * b;
    println(f2(6, 7));              // 42

    // 单捕获（按值捕获类型快照）
    let base = 40;
    let f3 = |x: i64| x + base;
    println(f3(2));                 // 42

    // 多捕获
    let ca = 30;
    let cb = 12;
    let f4 = |x: i64| x + ca + cb;
    println(f4(0));                 // 42

    // 无捕获
    let f5 = |x: i64| x * 2;
    println(f5(21));                // 42

    // 闭包值别名（聚合指针拷贝后仍可调用）
    let f6 = |x: i64| x + 1;
    let g6 = f6;
    println(g6(41));                // 42

    // 捕获 + 多参数
    let c7 = 10;
    let f7 = |a: i64, b: i64| a + b + c7;
    println(f7(15, 17));            // 42

    // 闭包值复用（多次调用）
    let f8 = |x: i64| x * 3;
    println(f8(10) + f8(4));        // 42

    // String 捕获（s9.len() = 5）
    let s9 = String::from("hello");
    let f9 = |x: i64| x + s9.len();
    println(f9(37));                // 42

    // String 实参升级（字面量 -> String）
    let f10 = |s: String| s.len() + 37;
    println(f10("hello"));          // 42

    // 闭包体复杂逻辑（if 表达式）
    let f11 = |x: i64| if x > 0 { x } else { -x };
    println(f11(-42));              // 42

    // 闭包体内局部变量（块表达式闭包体）
    let f12 = |x: i64| { let y = x * 2; y };
    println(f12(21));               // 42

    // 捕获字符串后拼接（闭包体内方法调用）
    let suffix = String::from("!");
    let f13 = |s: String| s + suffix;
    println(f13("rlyeh"));           // rlyeh!

    // 捕获数组变量做索引
    let table = [2, 4, 6];
    let f14 = |i: i64| table[i] * 7;
    println(f14(1));                // 28
}
