// benchmark: 256x256 f64 矩阵乘法 —— 与 matmul.rl 同逻辑
package main

import "fmt"

func main() {
	const n = 256
	const total = n * n
	a := make([]float64, total)
	b := make([]float64, total)
	c := make([]float64, total)
	for i := 0; i < total; i++ {
		a[i] = 1.0001
		b[i] = 1.0001
		c[i] = 0.0
	}
	for x := 0; x < n; x++ {
		for y := 0; y < n; y++ {
			var s float64
			for k := 0; k < n; k++ {
				s += a[x*n+k] * b[k*n+y]
			}
			c[x*n+y] = s
		}
	}
	var sum float64
	for i := 0; i < total; i++ {
		sum += c[i]
	}
	fmt.Printf("%.6f\n", sum)
}
