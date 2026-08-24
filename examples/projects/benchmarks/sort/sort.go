// benchmark: LCG 生成 5000 个 i64 排序 —— 与 sort.zeta 同逻辑
package main

import (
	"fmt"
	"sort"
)

func main() {
	const n = 5000
	v := make([]int64, n)
	x := int64(12345)
	for i := 0; i < n; i++ {
		x = x*6364136223846793005 + 1442695040888963407
		v[i] = x
	}
	sort.Slice(v, func(i, j int) bool { return v[i] < v[j] })
	fmt.Println(v[0])
	fmt.Println(v[n-1])
}
