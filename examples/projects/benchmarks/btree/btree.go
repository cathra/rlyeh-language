// 基准: btree —— 深度 15 完全二叉树（数组存储）+ 递归遍历求和
// 与 btree.rl 逻辑严格一致。输出 = 2166712927200
package main

import "fmt"

const total = 65535

func treeSum(a []int64, idx int64) int64 {
	if idx >= total {
		return 0
	}
	left := treeSum(a, idx*2+1)
	right := treeSum(a, idx*2+2)
	return a[idx] + left + right
}

func main() {
	a := make([]int64, total)
	for i := int64(0); i < total; i++ {
		a[i] = i*1009 + 17
	}
	fmt.Println(treeSum(a, 0))
}
