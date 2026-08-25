// 期望编译失败：调用未定义函数
// expect: function `foo` not found
fn main() {
    let r = foo(1, 2);
    println(r);
}
