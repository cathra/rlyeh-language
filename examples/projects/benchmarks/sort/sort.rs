// benchmark: LCG 生成 5000 个 i64 排序 —— 与 sort.zeta 同逻辑
fn main() {
    let n = 5000;
    let mut v: Vec<i64> = Vec::with_capacity(n as usize);
    let mut x: i64 = 12345;
    let mut i = 0;
    while i < n {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v.push(x);
        i = i + 1;
    }
    v.sort();
    println!("{}", v[0]);
    println!("{}", v[n - 1]);
}
