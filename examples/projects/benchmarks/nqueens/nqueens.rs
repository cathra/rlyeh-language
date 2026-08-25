// 基准: nqueens —— 12 皇后回溯搜索（纯整数 + 递归 + 分支）
// 与 nqueens.rl 逻辑严格一致。输出 = 14200
fn place(queens: &mut Vec<i64>, row: i64, n: i64) -> i64 {
    if row == n {
        return 1;
    }
    let mut total: i64 = 0;
    for col in 0..n {
        let mut ok = true;
        for r in 0..row {
            let q = queens[r as usize];
            if q == col || q - r == col - row || q + r == col + row {
                ok = false;
                break;
            }
        }
        if ok {
            queens.push(col);
            total += place(queens, row + 1, n);
            queens.pop();
        }
    }
    total
}

fn main() {
    let mut queens: Vec<i64> = Vec::new();
    println!("{}", place(&mut queens, 0, 12));
}
