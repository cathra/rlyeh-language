// benchmark: 20 万 insert + 20 万 get（LCG 键）—— 与 hashmap.rl 同逻辑
// Go 用内置 map（桶哈希 + 溢出链），与 Rlyeh/Rust 的标准 HashMap 对称。输出 = 19999900000
package main

import "fmt"

func main() {
	m := make(map[int64]int64, 1<<20)
	x := int64(12345)
	for i := 0; i < 200000; i++ {
		x = x*6364136223846793005 + 1442695040888963407
		m[x] = int64(i)
	}
	var sum int64
	y := int64(12345)
	for j := 0; j < 200000; j++ {
		y = y*6364136223846793005 + 1442695040888963407
		if v, ok := m[y]; ok {
			sum += v
		} else {
			sum += 1
		}
	}
	fmt.Println(sum)
}
