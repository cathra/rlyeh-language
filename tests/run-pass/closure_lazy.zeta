// H5 补全：闭包值惰性检查——从未调用的闭包（非注解/半注解）不检查闭包体，
// 即使闭包体引用了未定义变量或含有类型错误表达式，只要从未被调用即通过。
fn main() {
    // 从未调用：闭包体引用未定义变量也不报错
    let never = |x| undefined_var + 1;
    println(42);                            // 42

    // 从未调用：闭包体含类型错误表达式也不报错
    let never2 = |a, b: String| a * b;
    println(42);                            // 42

    // 半注解 + 从未调用
    let never3 = |x: i64, y| y + missing;
    println(42);                            // 42

    // 调用的闭包仍正常检查（对比：被调用时未定义变量必须报错——见
    // compile-fail 侧用例，这里用合法闭包确认调用路径不受惰性影响）
    let used = |x| x * 2;
    println(used(21));                      // 42
}
