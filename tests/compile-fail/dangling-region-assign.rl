// T-3：region 边界悬垂——区内 `r = &x` 将引用逃逸到区外变量，区外使用 `r`
// （其指向的区内局部 `x` 在 region 退出时释放）→ DanglingReference。
// expect: `x` does not live long enough
fn main() {
    let z = 5;
    let mut r = &z;
    region 'r {
        let x = 1;
        r = &x;
    }
    println(r);
}
