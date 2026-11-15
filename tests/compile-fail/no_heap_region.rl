// 无堆模式拒绝 region 区域分配（运行时 malloc/calloc）。
// flag: --no-heap
fn main() {
    region 'r {
        let x = 1;
    }
}
// expect: NH001
// expect: region
