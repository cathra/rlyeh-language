// W（SH-P2-10）：assert!/assert_eq!/assert_ne! 宏 desugar 为对内置 panic 的调用。
// 仅断言成立时不应 abort，并打印 7 收尾。
fn main() {
    assert!(1 < 2);
    assert_eq!(2 + 2, 4);
    assert_ne!(1, 2);
    assert_eq!(3, 3, "three should equal three");
    assert!(true, "should hold");
    println(7);
}
