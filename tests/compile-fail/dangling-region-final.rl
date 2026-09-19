// T-3：region 边界悬垂——region 块尾值是对区内局部 `x` 的引用，
// region 退出时 `x` 被批量释放，引用逃逸到区外变量 `r` → DanglingReference。
// expect: `x` does not live long enough
fn main() {
    let r = region 'r {
        let x = 1;
        &x
    };
    println(0);
}
