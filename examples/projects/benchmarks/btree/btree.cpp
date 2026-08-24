// 基准: btree —— 深度 15 完全二叉树（数组存储）+ 递归遍历求和
// 与 btree.zeta 逻辑严格一致。输出 = 2166712927200
#include <cstdio>
#include <cstdint>
#include <vector>

static int64_t tree_sum(const std::vector<int64_t> &a, int64_t idx, int64_t total) {
    if (idx >= total) return 0;
    int64_t left = tree_sum(a, idx * 2 + 1, total);
    int64_t right = tree_sum(a, idx * 2 + 2, total);
    return a[idx] + left + right;
}

int main() {
    const int64_t total = 65535;
    std::vector<int64_t> a;
    a.reserve((size_t)total);
    for (int64_t i = 0; i < total; i++) {
        a.push_back(i * 1009 + 17);
    }
    printf("%lld\n", tree_sum(a, 0, total));
    return 0;
}
