// G1 严格借用检查：悬垂引用——经引用拷贝别名逃逸出局部变量作用域（E0597，T-2）。
// 直接 `return &x` 已被既有 check_dangling_return 捕获；此处验证拷贝别名路径
// `let r = &x; let s = r; s` 同样触发 DanglingReference（报错名是被返回/逃逸的引用变量）。
// expect: `s` does not live long enough
fn g() -> &i64 {
    let x = 1;
    let r = &x;
    let s = r;
    s
}
fn main() {
    println(0);
}
