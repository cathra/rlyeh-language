// 期望编译失败：使用未定义变量
// expect: undefined variable
fn main() {
    let y = undefined_var + 1;
    println(y);
}
