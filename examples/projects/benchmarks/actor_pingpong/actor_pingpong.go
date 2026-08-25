// 基准: actor_pingpong —— 5 万次 goroutine 间同步往返（双无缓冲 channel）
// 与 actor_pingpong.rl 逻辑严格一致。输出 = 1250025000
package main

import "fmt"

const n = 50000

func worker(slotA <-chan int64, slotB chan<- int64) {
	var count int64
	for i := 0; i < n; i++ {
		<-slotA
		count++
		slotB <- count
	}
}

func main() {
	slotA := make(chan int64)
	slotB := make(chan int64)
	go worker(slotA, slotB)
	var sum int64
	for i := int64(0); i < n; i++ {
		slotA <- i
		sum += <-slotB
	}
	fmt.Println(sum)
}
