// 无堆模式拒绝 Box::new。
// flag: --no-heap
fn main() {
    let b = Box::new(1);
}
// expect: NH001
// expect: Box::new
