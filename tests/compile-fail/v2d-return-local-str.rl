// V2-D：返回局部 String 的 &str 应报 DanglingReference。
// expect: does not live long enough
fn bad() -> &str {
    let s = String::from("hi");
    s.as_str()
}
fn main() {
    let x = bad();
    println(x.len());
}
