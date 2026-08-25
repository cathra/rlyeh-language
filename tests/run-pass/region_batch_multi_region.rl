// 批量提升边界：region 在循环体内（RegionEnter/Exit in loop body）——
// 条件 3 拒绝提升，回落普通 bump，语义必须正确。
struct Big {
    a: i64,
    b: i64,
    c: i64,
    d: i64,
}

fn main() {
    let mut sum1 = 0;
    let mut sum2 = 0;
    let mut i = 0;
    while i < 1000 {
        region 'r1 {
            let x1 = Big { a: i, b: i + 1, c: i + 2, d: i + 3 } in 'r1;
            let x2 = Big { a: i, b: i + 1, c: i + 2, d: i + 3 } in 'r1;
            sum1 += x1.a + x1.b + x1.c + x1.d + x2.a + x2.b + x2.c + x2.d;
        }
        region 'r2 {
            let y1 = Big { a: i * 2, b: i, c: i, d: i } in 'r2;
            let y2 = Big { a: i * 2, b: i, c: i, d: i } in 'r2;
            sum2 += y1.a + y2.a;
        }
        i = i + 1;
    }
    println(sum1); // Σ(8i+12) i=0..999 = 8×499500 + 12000 = 4008000
    println(sum2); // Σ(4i) i=0..999 = 4×499500 = 1998000
}
