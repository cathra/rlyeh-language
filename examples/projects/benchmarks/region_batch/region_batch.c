// 基准: region_batch —— 100 万次循环 × 每次 4 个小对象分配（批量 bump 点）
// 对照: 手动 bump 分配器（与 region 语义对齐：一次性预分配 + 线性 bump，
//       退出一次性释放），对齐 region_batch.zeta 的 region 批量分配语义。
// 注: 旧版为 malloc/free（glibc tcache 复用同一小块内存，测的是 tcache 命中，
//     与 region 线性消耗带宽不对等），已替换为 bump 版。
// 输出: 2000497500000（与 region_batch.zeta 一致）
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    int64_t a, b, c, d;
} Big;

int main(void) {
    const size_t total = 1000000u * 4 * sizeof(Big);
    uint8_t *base = (uint8_t *)calloc(total, 1);
    uint8_t *cursor = base;
    int64_t sum = 0;
    for (int64_t i = 0; i < 1000000; i++) {
        // volatile 写/读：阻止 DSE/向量化（bump 内存不 escape 时 clang 会整体
        // 消除 store，测出纯计算假数据；volatile 强制真实内存带宽，对齐 region 语义）
        volatile Big *x1 = (volatile Big *)cursor; cursor += sizeof(Big);
        volatile Big *x2 = (volatile Big *)cursor; cursor += sizeof(Big);
        volatile Big *x3 = (volatile Big *)cursor; cursor += sizeof(Big);
        volatile Big *x4 = (volatile Big *)cursor; cursor += sizeof(Big);
        x1->a = i % 1000; x1->b = i; x1->c = i * 2; x1->d = i * 3;
        x2->a = i; x2->b = i + 1; x2->c = i + 2; x2->d = i + 3;
        x3->a = i * 2; x3->b = i; x3->c = i; x3->d = i;
        x4->a = i; x4->b = i; x4->c = i; x4->d = i;
        sum += x1->a + x2->a + x3->a + x4->a;
    }
    printf("%lld\n", (long long)sum);
    free(base);
    return 0;
}
