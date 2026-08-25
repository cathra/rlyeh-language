// G4 生命周期标注（MVP：语法接受 + 宽松检查；严格借用检查规划中）
struct Wrapper<'a> {
    inner: &'a i64,
}

fn longest<'a>(x: &'a i64, y: &'a i64) -> &'a i64 {
    if *x > *y { x } else { y }
}

fn main() {
    let a = 10;
    let b = 20;
    let m = longest(&a, &b);
    println(*m);                    // 20

    let w = Wrapper { inner: &a };
    println(*w.inner);              // 10

    // `&'a mut T` 形式
    let mut c = 5;
    let r: &'static mut i64 = &mut c;
    *r = 50;
    println(c);                     // 50

    // 生命周期与裸指针组合（G3/G4 互操作）
    let p: *const i64 = &b;
    println(*p);                    // 20
}
