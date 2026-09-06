// 嵌套元组解构 `let ((a, b), c) = e;` / `let (a, (b, c)) = e;` / 三层嵌套，
// 嵌套与通配符 / 异构字段混用。desugar 递归展开（每条内层元组取字段后再绑定）。
fn main() {
    // 二层嵌套（嵌套在前）
    let ((a, b), c) = ((1, 2), 3);
    println(a);          // 1
    println(b);          // 2
    println(c);          // 3
    println(a + b + c);  // 6

    // 嵌套在后
    let (x, (y, z)) = (10, (20, 30));
    println(x);          // 10
    println(y + z);      // 50

    // 嵌套 + 通配符
    let ((p, _), q) = ((4, 5), 6);
    println(p);          // 4
    println(q);          // 6

    // 三层嵌套
    let (((m, n), o), r) = (((7, 8), 9), 10);
    println(m + n + o + r); // 34

    // 异构元组嵌套（含堆分配字段）
    let ((s, t), u) = (("hello", 2), 3);
    println(s);          // hello
    println(t + u);      // 5

    if a == 1 && b == 2 && c == 3 && x == 10 && y == 20 && z == 30
       && p == 4 && q == 6 && m == 7 && n == 8 && o == 9 && r == 10 {
        println(1);      // 1
    } else {
        println(0);
    }
}
