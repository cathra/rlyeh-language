// U（SH-P2-8 M3）编译期失败：mem::take 要求首个参数为 &mut T。
fn main() {
    let a = 1;
    let old = mem::take(a);
    println(old);
}
// expect: mem::take 要求第一个参数为 &mut T
