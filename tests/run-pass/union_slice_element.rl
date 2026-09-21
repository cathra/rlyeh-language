// 开放问题②：切片元素为联合类型 `&[i64 | String]`（type-union §9）。
// 构造 `[i64 | String; N]` 数组 → 切片 → 索引 + 类型臂 match 收窄 + 再切片 + .len()。
// （切片迭代 `for x in xs.iter()` 受限于 pre-existing 的「指针元素切片迭代」通用问题，
//  见 type-union.md §9 ② 注记；此处用索引 + match 收窄这一主路径。）

fn first(xs: &[i64 | String]) -> i64 {
    match xs[0] {
        i64 => 1,
        String => 2,
        _ => 3,
    }
}

fn main() {
    let a: [i64 | String; 3] = [1i64, 2i64, 3i64];
    println(first(&a));        // 1：首元素为 i64

    let s = String::from("hi");
    let b: [i64 | String; 2] = [1i64, s];
    println(first(&b));        // 1：首元素仍为 i64

    // 逐元素 match 收窄 + 求和（i64 累加，String 取长度）
    let c: [i64 | String; 3] = [10i64, String::from("ab"), 20i64];
    let cs: &[i64 | String] = &c;
    let mut total = 0;
    let mut i = 0;
    while i < cs.len() {
        match cs[i] {
            i64 => total = total + i64,
            String => total = total + (String.len() as i64),
        }
        i = i + 1;
    }
    println(total);            // 32：10 + 2 + 20

    // 混合成员 + 切片再切片（sub-slice）
    let d: [i64 | String; 4] = [1i64, String::from("xyz"), 2i64, String::from("q")];
    let ds: &[i64 | String] = &d;
    let sub: &[i64 | String] = ds[1..<3];
    println(sub.len());        // 2：再切片 [1..<3]
    println(first(sub));       // 2：首元素为 String
}
