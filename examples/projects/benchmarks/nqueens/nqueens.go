// 基准: nqueens —— 12 皇后回溯搜索（纯整数 + 递归 + 分支）
// 与 nqueens.rl 逻辑严格一致。输出 = 14200
package main

import "fmt"

func place(queens []int64, row int64, n int64) int64 {
	if row == n {
		return 1
	}
	var total int64
	for col := int64(0); col < n; col++ {
		ok := true
		for r := int64(0); r < row; r++ {
			q := queens[r]
			if q == col || q-r == col-row || q+r == col+row {
				ok = false
				break
			}
		}
		if ok {
			queens[row] = col
			total += place(queens, row+1, n)
		}
	}
	return total
}

func main() {
	queens := make([]int64, 12)
	fmt.Println(place(queens, 0, 12))
}
