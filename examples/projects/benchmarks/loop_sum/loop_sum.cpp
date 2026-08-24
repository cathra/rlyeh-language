// benchmark: 1 亿次 i64 循环累加 —— 与 loop_sum.zeta 同逻辑
#include <cstdio>

int main() {
    long long s = 0;
    for (long long i = 0; i < 100000000; i++) s += i;
    printf("%lld\n", s);
    return 0;
}
