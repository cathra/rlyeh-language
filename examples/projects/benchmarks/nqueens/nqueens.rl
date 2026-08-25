// 基准: nqueens —— 12 皇后回溯搜索（纯整数 + 递归 + 分支）
// 测: 深度回溯搜索 + 递归调用 + 数组访问（替代 mandelbrot：Rlyeh MVP 的
//     `as f64` 数值转换尚未在 IR 层实现，见 benchmark_report.md 备注）
// 逻辑: 标准 n-queens 回溯（queens 数组存每行列号，剪枝检查对角线）。
//       输出 = 12 皇后解数 = 14200
fn place(queens: &mut Vec<i64>, row: i64, n: i64) -> i64 {
    if row == n {
        return 1;
    }
    let mut total = 0;
    let mut col = 0;
    while col < n {
        let mut ok = true;
        let mut r = 0;
        while r < row {
            let q = queens[r];
            if q == col || q - r == col - row || q + r == col + row {
                ok = false;
                break;
            }
            r = r + 1;
        }
        if ok {
            queens.push(col);
            total = total + place(queens, row + 1, n);
            queens.pop();
        }
        col = col + 1;
    }
    total
}

fn main() {
    let mut queens = Vec::new();
    println(place(&mut queens, 0, 12));
}
