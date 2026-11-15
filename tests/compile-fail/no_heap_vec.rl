// 无堆模式拒绝 Vec 堆容器。
// flag: --no-heap
fn main() {
    let v: Vec<i64> = Vec::new();
}
// expect: NH001
// expect: Vec
