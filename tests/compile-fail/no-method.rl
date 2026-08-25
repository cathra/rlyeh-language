// 期望编译失败：方法不存在
// expect: function `String::missing_method` not found
fn main() {
    let s = String::from("hi");
    let n = s.missing_method();
    println(n);
}
