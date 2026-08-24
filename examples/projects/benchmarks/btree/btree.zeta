// 基准: btree —— 深度 15 完全二叉树（数组存储）+ 递归遍历求和
// 测: 大数组分配 + 层次随机访问 + 递归调用深度
// 逻辑: 65535 个节点（索引 i 的值为 i*1009+17），自根递归求和。
//       输出 = Σ(i*1009+17) for i in 0..65535
fn tree_sum(a: &Vec<i64>, idx: i64, total: i64) -> i64 {
    if idx >= total {
        return 0;
    }
    let left = tree_sum(a, idx * 2 + 1, total);
    let right = tree_sum(a, idx * 2 + 2, total);
    a[idx] + left + right
}

fn main() {
    let total = 65535;
    let mut a = Vec::with_capacity(total);
    let mut i = 0;
    while i < total {
        a.push(i * 1009 + 17);
        i = i + 1;
    }
    println(tree_sum(&a, 0, total));
}
