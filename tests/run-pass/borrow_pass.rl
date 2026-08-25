// G1 严格借用检查：合法借用模式（与 Rust NLL 语义对齐）。
// 覆盖：共享借用共存、借用结束后写原变量（NLL 释放）、块内借用、
// 参数引用返回（生命周期 elision）、临时借用实参、ref 模式与借用检查共存。
fn max_ref<'a>(a: &i64, b: &i64) -> &i64 {
    if *a > *b { a } else { b }
}
fn main() {
    // 1. 共享借用可共存
    let x = 10;
    let r1 = &x;
    let r2 = &x;
    println(*r1 + *r2);              // 20
    println(r1);                     // 10：引用参数自动剥层（G1）

    // 2. NLL：借用结束后（引用变量不再使用）可写原变量
    let mut y = 5;
    let ry = &y;
    println(*ry);                    // 5
    y = 7;                           // ry 最后使用后 → 允许
    println(y);                      // 7

    // 3. 块内借用：块结束释放，外层可写
    let mut z = 1;
    if true {
        let rz = &z;
        println(*rz);                // 1
    }
    z = 9;
    println(z);                      // 9

    // 4. 临时借用作实参（仅调用语句活跃）
    println(max_ref(&y, &z));        // 9

    // 5. 参数引用返回合法（借用源为参数，不悬垂）
    let q = max_ref(&y, &z);
    println(*q);                     // 9

    // 6. ref 模式与借用检查共存（ref 指向 match 的临时拷贝，原变量不受影响）
    let s = String::from("rlyeh");
    match s {
        ref r => println(r.len()),   // 4
    }
    println(s.len());                // 4：s 未被借用
}
