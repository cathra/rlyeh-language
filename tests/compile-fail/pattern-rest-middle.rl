// 中间 `..` 暂不支持：剩余模式须位于末位。
fn main() {
    let (a, .., b) = (1, 2, 3);
    println(a);
    println(b);
}
