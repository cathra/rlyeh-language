// 基准: region_alloc —— 100 万次小对象分配 + 释放（new/delete）
// 与 region_alloc.zeta 逻辑严格一致。输出 = 499500000
// 注: Zeta 侧用 region 批量分配（区域退出一次释放），本侧为每次 new+delete。
#include <cstdio>
#include <cstdint>

struct Big {
    int64_t a, b, c, d;
};

int main() {
    int64_t sum = 0;
    for (int i = 0; i < 1000000; i++) {
        Big *o = new Big{i % 1000, i, i * 2, i * 3};
        sum += o->a;
        delete o;
    }
    printf("%lld\n", sum);
    return 0;
}
