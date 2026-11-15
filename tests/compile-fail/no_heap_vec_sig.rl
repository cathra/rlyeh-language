// 无堆模式拒绝函数签名中的堆容器类型（即使未构造）。
// flag: --no-heap
fn f(v: Vec<i64>) -> i64 {
    1
}

fn main() {
    println(1);
}
// expect: NH001
// expect: Vec
