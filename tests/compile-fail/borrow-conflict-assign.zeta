// G1 严格借用检查：赋值（写）被借用中的变量（E0506 类似）。
// `r` 在赋值后仍被使用（println(*r)），借用活跃期内写入 `x` → 冲突。
// expect: cannot assign to `x` because it is borrowed
fn main() {
    let mut x = 5;
    let r = &x;
    x = 7;
    println(*r);
}
