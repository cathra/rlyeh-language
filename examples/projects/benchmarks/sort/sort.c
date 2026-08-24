// benchmark: LCG 生成 5000 个 i64 排序 —— 与 sort.zeta 同逻辑
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

static int cmp(const void *pa, const void *pb) {
    int64_t a = *(const int64_t *)pa;
    int64_t b = *(const int64_t *)pb;
    return (a > b) - (a < b);
}

int main(void) {
    const int n = 5000;
    int64_t *v = malloc(n * sizeof(int64_t));
    int64_t x = 12345;
    for (int i = 0; i < n; i++) {
        x = (int64_t)((uint64_t)x * 6364136223846793005ULL + 1442695040888963407ULL);
        v[i] = x;
    }
    qsort(v, n, sizeof(int64_t), cmp);
    printf("%lld\n%lld\n", v[0], v[n - 1]);
    free(v);
    return 0;
}
