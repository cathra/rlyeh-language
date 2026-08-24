// 基准: region_alloc —— 100 万次小对象分配（Go GC 回收）
// 与 region_alloc.zeta 逻辑严格一致。输出 = 499500000
// 注: Zeta 侧用 region 批量分配（区域退出一次释放），本侧每次 new 后由 GC 回收。
package main

import "fmt"

type Big struct {
	a, b, c, d int64
}

func main() {
	var sum int64
	for i := int64(0); i < 1000000; i++ {
		o := &Big{a: i % 1000, b: i, c: i * 2, d: i * 3}
		sum += o.a
	}
	fmt.Println(sum)
}
