// 基准: region_alloc —— 100 万次小对象分配 + 释放（Box 堆分配）
// 与 region_alloc.zeta 逻辑严格一致。输出 = 499500000
// 注: Zeta 侧用 region 批量分配（区域退出一次释放），本侧为每次 Box::new + drop。
struct Big {
    a: i64,
    b: i64,
    c: i64,
    d: i64,
}

fn main() {
    let mut sum: i64 = 0;
    for i in 0..1_000_000i64 {
        let o = Box::new(Big {
            a: i % 1000,
            b: i,
            c: i * 2,
            d: i * 3,
        });
        sum += o.a;
    }
    println!("{}", sum);
}
