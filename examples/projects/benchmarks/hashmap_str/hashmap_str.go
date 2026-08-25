// 基准: hashmap_str —— 1 万条字符串键哈希表插入与查询
// 与 hashmap_str.rl 逻辑严格一致。输出 = 49995000
// 实现: Go 内置 map + fmt.Sprintf 键构造（每次堆分配，与 Rlyeh format! 对称）
package main

import "fmt"

const n = 10000

func main() {
	m := make(map[string]int64, 1<<14)
	for i := 0; i < n; i++ {
		m[fmt.Sprintf("key_%d", i)] = int64(i)
	}
	var sum int64
	for i := 0; i < n; i++ {
		if v, ok := m[fmt.Sprintf("key_%d", i)]; ok {
			sum += v
		} else {
			sum -= 1
		}
	}
	fmt.Println(sum)
}
