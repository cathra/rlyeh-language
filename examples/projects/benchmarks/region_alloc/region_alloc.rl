// 基准: region_alloc —— 100 万次小对象分配（region 批量 bump 分配）
// 测: 分配器吞吐（Rlyeh region L1/L3：批量分配 + 区域退出一次性释放）
// 逻辑: region 'r 内循环创建 100 万个 Big 对象（32 字节），累加 o.a。
//       输出 = Σ(i%1000) for i in 0..1000000 = 499,500,000
// 注: Rlyeh 侧用 region 批量分配（区域退出才释放），C/C++/Rust/Swift 侧为
//     每次分配+释放（malloc/free、new/delete、Box、class）——反映不同内存
//     管理模型的分配吞吐差异（语义差异见 README）。
struct Big {
    a: i64,
    b: i64,
    c: i64,
    d: i64,
}

fn main() {
    region 'r {
        let mut sum = 0;
        let mut i = 0;
        while i < 1000000 {
            let o = Big { a: i % 1000, b: i, c: i * 2, d: i * 3 } in 'r;
            sum = sum + o.a;
            i = i + 1;
        }
        println(sum);
    }
}
