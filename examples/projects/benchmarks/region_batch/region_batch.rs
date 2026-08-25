// 对照实现: region_batch.rs —— 手动 bump（与 region 语义对齐：一次性预分配 + 线性分配）
// write_volatile/read_volatile：阻止 LLVM DSE（bump 内存不 escape 时会被整体消除，
// 测出纯计算假数据；volatile 强制真实内存带宽，对齐 region_batch.rl）。
// 输出 = 2000497500000
struct Big {
    a: i64,
    b: i64,
    c: i64,
    d: i64,
}

fn main() {
    let total = 1_000_000usize * 4 * std::mem::size_of::<Big>();
    let mut buf = vec![0u8; total];
    let mut cursor = buf.as_mut_ptr() as *mut Big;
    let mut sum: i64 = 0;
    for i in 0i64..1_000_000 {
        unsafe {
            let x1 = cursor; cursor = cursor.add(1);
            let x2 = cursor; cursor = cursor.add(1);
            let x3 = cursor; cursor = cursor.add(1);
            let x4 = cursor; cursor = cursor.add(1);
            std::ptr::write_volatile(&mut (*x1).a, i % 1000);
            std::ptr::write_volatile(&mut (*x1).b, i);
            std::ptr::write_volatile(&mut (*x1).c, i * 2);
            std::ptr::write_volatile(&mut (*x1).d, i * 3);
            std::ptr::write_volatile(&mut (*x2).a, i);
            std::ptr::write_volatile(&mut (*x2).b, i + 1);
            std::ptr::write_volatile(&mut (*x2).c, i + 2);
            std::ptr::write_volatile(&mut (*x2).d, i + 3);
            std::ptr::write_volatile(&mut (*x3).a, i * 2);
            std::ptr::write_volatile(&mut (*x3).b, i);
            std::ptr::write_volatile(&mut (*x3).c, i);
            std::ptr::write_volatile(&mut (*x3).d, i);
            std::ptr::write_volatile(&mut (*x4).a, i);
            std::ptr::write_volatile(&mut (*x4).b, i);
            std::ptr::write_volatile(&mut (*x4).c, i);
            std::ptr::write_volatile(&mut (*x4).d, i);
            sum += std::ptr::read_volatile(&(*x1).a)
                + std::ptr::read_volatile(&(*x2).a)
                + std::ptr::read_volatile(&(*x3).a)
                + std::ptr::read_volatile(&(*x4).a);
        }
    }
    println!("{}", sum);
}
