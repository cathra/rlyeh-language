// 基准: region_batch —— 100 万次循环 × 每次 4 个小对象分配（批量 bump 点）
// 测: 批量提升（循环内多个 region bump 点聚合为单次溢出检查 + 寄存器游标）
// 逻辑: region 'r 内循环每次分配 4 个 Big（32B），累加各首字段。
//       输出 = Σ(i%1000) + 3*Σ(i) + Σ(2i) = 499,500,000 + 3*499,999,500,000 + 999,999,000,000
//             = 200,049,750,0000
// 注: 4 个 bump 点全部为直接构造（Direct）且无副作用穿插 → codegen 批量提升
//     聚合为一次溢出检查 + 4 个 gep 派生（详见 docs/memory-model.md 附录 A.6）。
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
            let x1 = Big { a: i % 1000, b: i, c: i * 2, d: i * 3 } in 'r;
            let x2 = Big { a: i, b: i + 1, c: i + 2, d: i + 3 } in 'r;
            let x3 = Big { a: i * 2, b: i, c: i, d: i } in 'r;
            let x4 = Big { a: i, b: i, c: i, d: i } in 'r;
            sum = sum + x1.a + x2.a + x3.a + x4.a;
            i = i + 1;
        }
        println(sum);
    }
}
