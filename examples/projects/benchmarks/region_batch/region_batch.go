// 对照实现: region_batch.go —— 手动 bump（与 region 语义对齐：一次性预分配 + 线性分配）
// Go 编译器不做跨语句 store 消除，指针写即真实内存带宽。
// 输出 = 2000497500000
package main

import (
	"fmt"
	"unsafe"
)

type Big struct {
	a, b, c, d int64
}

func main() {
	buf := make([]byte, 1000000*4*int(unsafe.Sizeof(Big{})))
	cur := unsafe.Pointer(&buf[0])
	var sum int64
	for i := int64(0); i < 1000000; i++ {
		x1 := (*Big)(cur); cur = unsafe.Add(cur, unsafe.Sizeof(Big{}))
		x2 := (*Big)(cur); cur = unsafe.Add(cur, unsafe.Sizeof(Big{}))
		x3 := (*Big)(cur); cur = unsafe.Add(cur, unsafe.Sizeof(Big{}))
		x4 := (*Big)(cur); cur = unsafe.Add(cur, unsafe.Sizeof(Big{}))
		x1.a = i % 1000; x1.b = i; x1.c = i * 2; x1.d = i * 3
		x2.a = i; x2.b = i + 1; x2.c = i + 2; x2.d = i + 3
		x3.a = i * 2; x3.b = i; x3.c = i; x3.d = i
		x4.a = i; x4.b = i; x4.c = i; x4.d = i
		sum += x1.a + x2.a + x3.a + x4.a
	}
	fmt.Println(sum)
}
