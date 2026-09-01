// M2（SH-P0-5）：元组解构绑定 `let (a, b) = e;` / `let (a, _, c) = e;`
// + M3：函数多返回值 `fn f() -> (i64, String)` 经解构接收。
// desugar：临时变量承载元组值（init 只求值一次）+ 各元素按位置 `FieldGet`
// 取出后分别绑定（与 `t.f0` 字段访问同构）。
fn split_name(s: String) -> (String, i64) {
    (s, s.len())
}

fn main() {
    // 基本解构
    let (a, b) = (1, 2);
    println(a);              // 1
    println(b);              // 2
    println(a + b);          // 3

    // 通配符 `_`：跳过对应位置（仍占用下标）
    let (x, _, z) = (10, 20, 30);
    println(x);              // 10
    println(z);              // 30

    // 异构元组（含堆分配字段）
    let (num, s) = (7, "rlyeh");
    println(num);            // 7
    println(s);              // rlyeh

    // 多返回值 + 解构接收：init 为函数调用（只求值一次）
    let (name, len) = split_name(String::from("rlyeh"));
    println(len);            // 5

    // `mut` 解构：整体可变的元组绑定
    let mut (p, q) = (1, 2);
    p = 10;
    println(p + q);          // 12

    // 三元组完整解构
    let (i, j, k) = (3, 4, 5);
    println(i * j * k);      // 60

    if a == 1 && b == 2 && x == 10 && z == 30 && len == 5 && p == 10 {
        println(1);          // 1
    } else {
        println(0);
    }
}
