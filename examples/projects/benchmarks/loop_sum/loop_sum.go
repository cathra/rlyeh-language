// benchmark: 1 亿次 i64 循环累加 —— 与 loop_sum.zeta 同逻辑。输出 = 4999999950000000
package main

import "fmt"

func main() {
	var s int64
	for i := int64(0); i < 100000000; i++ {
		s += i
	}
	fmt.Println(s)
}
