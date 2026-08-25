// 基准: btree —— 深度 15 完全二叉树（数组存储）+ 递归遍历求和
// 与 btree.rl 逻辑严格一致。输出 = 2166712927200
let total: Int64 = 65535

func treeSum(_ a: [Int64], _ idx: Int64, _ total: Int64) -> Int64 {
    if idx >= total { return 0 }
    let left = treeSum(a, idx * 2 + 1, total)
    let right = treeSum(a, idx * 2 + 2, total)
    return a[Int(idx)] + left + right
}

var a = [Int64]()
a.reserveCapacity(Int(total))
for i in 0..<total {
    a.append(i * 1009 + 17)
}
print(treeSum(a, 0, total))
