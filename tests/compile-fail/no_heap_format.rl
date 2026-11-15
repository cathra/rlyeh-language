// 无堆模式拒绝 format!（内部 String builder 需要堆）。
// flag: --no-heap
fn main() {
    let s = format!("{}", 1);
}
// expect: NH001
// expect: format!
