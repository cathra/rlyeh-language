// M2：嵌套元组解构 `let ((a, b), c) = e;` 现已支持（元素模式接受嵌套元组，
// desugar 递归展开：外层取字段得内层元组值，再按位置绑定）。
fn main() {
    let ((a, b), c) = ((1, 2), 3);
    println(a);          // 1
    println(b);          // 2
    println(c);          // 3
    println(a + b + c);  // 6
}
