// 对照实现: region_batch.cpp —— 手动 bump（与 region 语义对齐：一次性预分配 + 线性分配）
// volatile 写/读：阻止 DSE（bump 内存不 escape 时 clang 会整体消除 store，
// 测出纯计算假数据；volatile 强制真实内存带宽，对齐 region_batch.zeta）。
// 输出 = 2000497500000
#include <cstdint>
#include <cstdio>
#include <vector>

struct Big {
    int64_t a, b, c, d;
};

int main() {
    const size_t total = 1000000u * 4 * sizeof(Big);
    std::vector<uint8_t> buf(total);
    uint8_t *cursor = buf.data();
    int64_t sum = 0;
    for (int64_t i = 0; i < 1000000; i++) {
        volatile Big *x1 = reinterpret_cast<volatile Big *>(cursor); cursor += sizeof(Big);
        volatile Big *x2 = reinterpret_cast<volatile Big *>(cursor); cursor += sizeof(Big);
        volatile Big *x3 = reinterpret_cast<volatile Big *>(cursor); cursor += sizeof(Big);
        volatile Big *x4 = reinterpret_cast<volatile Big *>(cursor); cursor += sizeof(Big);
        x1->a = i % 1000; x1->b = i; x1->c = i * 2; x1->d = i * 3;
        x2->a = i; x2->b = i + 1; x2->c = i + 2; x2->d = i + 3;
        x3->a = i * 2; x3->b = i; x3->c = i; x3->d = i;
        x4->a = i; x4->b = i; x4->c = i; x4->d = i;
        sum += x1->a + x2->a + x3->a + x4->a;
    }
    std::printf("%lld\n", static_cast<long long>(sum));
    return 0;
}
