// benchmark: fib(30) —— 与 fib.rl 同逻辑。输出 = 832040
package main

import "fmt"

func fib(n int64) int64 {
	if n < 2 {
		return n
	}
	return fib(n-1) + fib(n-2)
}

func main() {
	fmt.Println(fib(30))
}
