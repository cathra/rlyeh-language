// 开放问题② 负例：切片元素为联合时，仅当元素类型完全相同方可 unsize coerce。
// `&[i64; 3]` 不可 coerce 成 `&[i64 | String]`（步长不同，按值 reinterpret 属未定义
// 行为，见 type-union §9 ②）。此处应被类型检查拒绝（TC018）。
// expect: TC018
// expect: &[i64 | String]

fn first(xs: &[i64 | String]) -> i64 {
    match xs[0] { i64 => 1, String => 2, _ => 3 }
}

fn main() {
    let a = [1i64, 2i64, 3i64];
    println(first(&a));
}
