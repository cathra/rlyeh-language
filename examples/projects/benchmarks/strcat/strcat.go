// benchmark: 字符串拼接 10 万次 —— 与 strcat.zeta 同逻辑（容量翻倍动态缓冲）
// Go 用字节切片 append（容量不足自动翻倍扩容），与 C 的 realloc 语义一致。输出 = 200000
package main

import "fmt"

func main() {
	buf := make([]byte, 0, 16)
	for i := 0; i < 100000; i++ {
		buf = append(buf, 'a', 'b')
	}
	fmt.Println(len(buf))
}
