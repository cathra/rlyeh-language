// 无堆模式拒绝 String 堆容器。
// flag: --no-heap
fn main() {
    let s = String::from("secret");
}
// expect: NH001
// expect: String::from
