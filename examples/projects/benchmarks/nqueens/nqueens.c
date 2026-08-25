// 基准: nqueens —— 12 皇后回溯搜索（纯整数 + 递归 + 分支）
// 与 nqueens.rl 逻辑严格一致。输出 = 14200
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

static int64_t place(int64_t *queens, int64_t row, int64_t n) {
    if (row == n) return 1;
    int64_t total = 0;
    for (int64_t col = 0; col < n; col++) {
        int ok = 1;
        for (int64_t r = 0; r < row; r++) {
            int64_t q = queens[r];
            if (q == col || q - r == col - row || q + r == col + row) {
                ok = 0;
                break;
            }
        }
        if (ok) {
            queens[row] = col;
            total += place(queens, row + 1, n);
        }
    }
    return total;
}

int main(void) {
    int64_t *queens = malloc(sizeof(int64_t) * 12);
    printf("%lld\n", place(queens, 0, 12));
    free(queens);
    return 0;
}
