// G1 严格借用检查：同一时刻两个可变借用（E0499）。
// `r` 在 `r2` 之后仍被使用（*r = 10）→ 借用互斥冲突。
// expect: cannot mutably borrow `x` because it is already borrowed as mutable
fn main() {
    let mut x = 5;
    let r = &mut x;
    let r2 = &mut x;
    *r = 10;
    println(x);
}
