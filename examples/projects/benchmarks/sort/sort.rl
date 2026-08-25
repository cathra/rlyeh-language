// benchmark: LCG 生成 5000 个 i64 排序 —— 排序（当前 Vec::sort_by 为 O(n^2) 选择排序）
fn cmp(a: i64, b: i64) -> i64 {
    if a < b {
        return -1;
    }
    if a > b {
        return 1;
    }
    0
}

fn main() {
    let n = 5000;
    let mut v: Vec<i64> = Vec::new();
    let mut x = 12345;
    let mut i = 0;
    while i < n {
        x = x * 6364136223846793005 + 1442695040888963407;
        v.push(x);
        i = i + 1;
    }
    v.sort_by(cmp);
    println(v[0]);
    println(v[n - 1]);
}
