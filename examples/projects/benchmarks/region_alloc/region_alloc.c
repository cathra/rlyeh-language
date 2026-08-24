// 基准: region_alloc —— 100 万次小对象分配 + 释放（malloc/free）
// 与 region_alloc.zeta 逻辑严格一致。输出 = 499500000
// 注: Zeta 侧用 region 批量分配（区域退出一次释放），本侧为每次 malloc+free。
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

typedef struct {
    int64_t a, b, c, d;
} Big;

int main(void) {
    int64_t sum = 0;
    for (int i = 0; i < 1000000; i++) {
        Big *o = malloc(sizeof(Big));
        o->a = i % 1000;
        o->b = i;
        o->c = i * 2;
        o->d = i * 3;
        sum += o->a;
        free(o);
    }
    printf("%lld\n", sum);
    return 0;
}
