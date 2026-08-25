// 基准: dyn_dispatch —— 2000 万次接口方法多态分派
// 与 dyn_dispatch.rl 逻辑严格一致。输出 = 70000000
// 实现: Go interface（itab 间接调用），与 Rlyeh dyn Trait 胖指针 + vtable 分派对称
package main

import "fmt"

type Shape interface {
	sides() int64
}

type Tri struct{ n int64 }
type Quad struct{ n int64 }

func (t *Tri) sides() int64  { return t.n + 3 }
func (q *Quad) sides() int64 { return q.n + 4 }

func main() {
	var d1 Shape = &Tri{0}
	var d2 Shape = &Quad{0}
	var sum int64
	for i := 0; i < 10000000; i++ {
		sum += d1.sides()
		sum += d2.sides()
	}
	fmt.Println(sum)
}
