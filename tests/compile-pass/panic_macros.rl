// W（SH-P2-10）：panic! / unreachable! / todo! / assert!(false) 编译期应可编译
// （运行时触发终止，故置于 compile-pass，不执行）。
fn main() {
    panic!("boom");
    unreachable!();
    todo!();
    assert!(1 == 2);
}
