// G1 严格借用检查：共享借用活跃期内的可变借用（E0502）。
// `r` 在 `&mut x` 之后仍被使用（println(*r)）→ 冲突。
// expect: cannot mutably borrow `x` because it is already borrowed as shared
fn main() {
    let mut x = 5;
    let r = &x;
    let r2 = &mut x;
    println(*r);
}
