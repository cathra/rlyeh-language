// 批量提升边界：循环体含非 latch 块（if 分支）内的分配——
// bump 不全在 latch，条件 5 拒绝提升；latch 内连续 bump 仍走
// P3 批量 bump 聚合，语义必须正确。
struct Big {
    a: i64,
    b: i64,
    c: i64,
    d: i64,
}

fn main() {
    let mut sum = 0;
    let mut i = 0;
    region 'r {
        while i < 1000 {
            // latch 内连续 2 个 bump（批量 bump 候选）
            let x1 = Big { a: i, b: i, c: i, d: i } in 'r;
            let x2 = Big { a: i, b: i, c: i, d: i } in 'r;
            // 非 latch 块（if 分支）内的 bump——整体回落不提升
            if i % 2 == 0 {
                let y = Big { a: i * 2, b: i, c: i, d: i } in 'r;
                sum += y.a;
            }
            sum += x1.a + x2.a;
            i = i + 1;
        }
    }
    println(sum); // Σ2i + 偶数 i 的 2i = 999000 + 499000 = 1498000
}
