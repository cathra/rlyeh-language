// M2（SH-P0-1）：对不可变 `static` 赋值必须报错。
// expect: cannot assign to immutable static
static N: i64 = 1;
fn main() {
    N = 5;
}
