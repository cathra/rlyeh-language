// 基准: nqueens —— 12 皇后回溯搜索（纯整数 + 递归 + 分支）
// 与 nqueens.rl 逻辑严格一致。输出 = 14200
#include <cstdio>
#include <cstdint>
#include <vector>

static int64_t place(std::vector<int64_t> &queens, int64_t row, int64_t n) {
    if (row == n) return 1;
    int64_t total = 0;
    for (int64_t col = 0; col < n; col++) {
        bool ok = true;
        for (int64_t r = 0; r < row; r++) {
            int64_t q = queens[(size_t)r];
            if (q == col || q - r == col - row || q + r == col + row) {
                ok = false;
                break;
            }
        }
        if (ok) {
            queens.push_back(col);
            total += place(queens, row + 1, n);
            queens.pop_back();
        }
    }
    return total;
}

int main() {
    std::vector<int64_t> queens;
    queens.reserve(12);
    printf("%lld\n", place(queens, 0, 12));
    return 0;
}
