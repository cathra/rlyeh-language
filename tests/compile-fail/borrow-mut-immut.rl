// G1 严格借用检查：对不可变绑定取可变引用（E0596）。
// expect: cannot borrow `x` as mutable, as it is not declared as mutable
fn main() {
    let x = 5;
    let r = &mut x;
    *r = 6;
    println(x);
}
