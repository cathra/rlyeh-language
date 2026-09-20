// M2（SH-P0-1 E3）：在 `unsafe` 块外对 `static mut` 赋值必须报错。
// expect: must be inside an `unsafe` block
static mut M: i64 = 1;
fn main() {
    M = 5;
}
