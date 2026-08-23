// G1 严格借用检查：可变借用活跃期内的共享借用（E0502）。
// `r` 在 `&x` 之后仍被使用（*r = 10）→ 冲突。
// expect: cannot borrow `x` as shared because it is already borrowed as mutable
fn main() {
    let mut x = 5;
    let r = &mut x;
    let r2 = &x;
    *r = 10;
    println(x);
}
