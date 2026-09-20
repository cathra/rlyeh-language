// M2（SH-P0-1）：`static` / `static mut` 初始值必须是编译期常量。
// expect: not a compile-time constant
fn get() -> i64 { 7 }
static mut R: i64 = get();
fn main() {
}
