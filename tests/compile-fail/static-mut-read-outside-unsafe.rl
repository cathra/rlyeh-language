// M2（SH-P0-1 E3）：在 `unsafe` 块外读取 `static mut` 必须报错。
// expect: must be inside an `unsafe` block
static mut M: i64 = 1;
fn main() {
    let x = M;
}
