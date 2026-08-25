// 期望编译失败：`?` 仅支持 Option/Result 类型
// expect: `?` 运算符仅支持 Option/Result 类型
fn main() {
    let x = 42;
    let y = x?;
    println(y);
}
