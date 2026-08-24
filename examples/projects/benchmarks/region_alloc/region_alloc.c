// 基准: region_alloc —— 100 万次小对象分配（region 语义对照）
// 对照: 手动 bump 分配器（一次性预分配 + 线性 bump，退出一次性释放），
//       对齐 region_alloc.zeta 的 region 批量分配语义。
// 注: 旧版为 malloc/free（glibc tcache 复用同一小块内存，测的是 tcache 命中，
//     与 region 线性消耗带宽不对等），已替换为 bump 版。
// 输出: 499500000（与 region_alloc.zeta 一致）
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

typedef struct {
    int64_t a, b, c, d;
} Big;

int main(void) {
    const size_t total = 1000000u * sizeof(Big);
    uint8_t *base = (uint8_t *)calloc(total, 1);
    uint8_t *cursor = base;
    int64_t sum = 0;
    for (int64_t i = 0; i < 1000000; i++) {
        // volatile 写/读：阻止 DSE/向量化（bump 内存不 escape 时 clang 会整体
        // 消除 store，测出纯计算假数据；volatile 强制真实内存带宽，对齐 region 语义）
        volatile Big *o = (volatile Big *)cursor; cursor += sizeof(Big);
        o->a = i % 1000;
        o->b = i;
        o->c = i * 2;
        o->d = i * 3;
        sum += o->a;
    }
    printf("%lld\n", (long long)sum);
    free(base);
    return 0;
}
