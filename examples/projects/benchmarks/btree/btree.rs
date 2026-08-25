// 基准: btree —— 深度 15 完全二叉树（数组存储）+ 递归遍历求和
// 与 btree.rl 逻辑严格一致。输出 = 2166712927200
fn tree_sum(a: &[i64], idx: i64, total: i64) -> i64 {
    if idx >= total {
        return 0;
    }
    let left = tree_sum(a, idx * 2 + 1, total);
    let right = tree_sum(a, idx * 2 + 2, total);
    a[idx as usize] + left + right
}

fn main() {
    let total: i64 = 65535;
    let mut a: Vec<i64> = Vec::with_capacity(total as usize);
    for i in 0..total {
        a.push(i * 1009 + 17);
    }
    println!("{}", tree_sum(&a, 0, total));
}
