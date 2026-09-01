// SH-P0-1：unsafe 块 + 裸指针（*const T / *mut T）读写与索引。
// 验证运行时表达力地基：可在 unsafe 作用域内手动操作裸指针。
fn main() {
    let arr = [10, 20, 30];
    let p: *mut i64 = &arr[0];
    unsafe {
        *p = 99;     // 解引用写回首元素
        p[1] = 7;    // 裸指针索引写
    }
    println(arr[0]);
    println(arr[1]);
    let q: *const i64 = &arr[0];
    unsafe {
        println(*q);   // 解引用读
        println(q[2]); // 裸指针索引读
    }
}
