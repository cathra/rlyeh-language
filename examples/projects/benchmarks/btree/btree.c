// 基准: btree —— 深度 15 完全二叉树（数组存储）+ 递归遍历求和
// 与 btree.rl 逻辑严格一致。输出 = 2166712927200
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

#define TOTAL 65535

static int64_t tree_sum(const int64_t *a, int64_t idx, int64_t total) {
    if (idx >= total) return 0;
    int64_t left = tree_sum(a, idx * 2 + 1, total);
    int64_t right = tree_sum(a, idx * 2 + 2, total);
    return a[idx] + left + right;
}

int main(void) {
    int64_t *a = malloc(sizeof(int64_t) * TOTAL);
    for (int64_t i = 0; i < TOTAL; i++) {
        a[i] = i * 1009 + 17;
    }
    printf("%lld\n", tree_sum(a, 0, TOTAL));
    free(a);
    return 0;
}
