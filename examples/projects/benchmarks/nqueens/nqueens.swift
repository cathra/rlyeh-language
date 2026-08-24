// 基准: nqueens —— 12 皇后回溯搜索（纯整数 + 递归 + 分支）
// 与 nqueens.zeta 逻辑严格一致。输出 = 14200
func place(_ queens: inout [Int64], _ row: Int64, _ n: Int64) -> Int64 {
    if row == n { return 1 }
    var total: Int64 = 0
    for col in 0..<n {
        var ok = true
        for r in 0..<row {
            let q = queens[Int(r)]
            if q == col || q - r == col - row || q + r == col + row {
                ok = false
                break
            }
        }
        if ok {
            queens.append(col)
            total += place(&queens, row + 1, n)
            queens.removeLast()
        }
    }
    return total
}

var queens = [Int64]()
print(place(&queens, 0, 12))
