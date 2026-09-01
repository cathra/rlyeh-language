// SH-P0-1 E3：在 `unsafe` 块外调用 extern 函数必须报错。
// expect: call to extern function
fn main() {
    let _p = calloc(1, 8);
    println("ok");
}
