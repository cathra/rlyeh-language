// K4 追踪 GC：Gc::new 参数错误
fn main() {
    // 缺参数：Gc::new 需要恰好 1 个实参
    let g = Gc::new();
    println(*g);
}
