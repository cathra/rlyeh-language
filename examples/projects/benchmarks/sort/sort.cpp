// benchmark: LCG 生成 5000 个 i64 排序 —— 与 sort.zeta 同逻辑
#include <cstdio>
#include <vector>
#include <algorithm>

int main() {
    const int n = 5000;
    std::vector<long long> v;
    v.reserve(n);
    long long x = 12345;
    for (int i = 0; i < n; i++) {
        x = (long long)((unsigned long long)x * 6364136223846793005ULL + 1442695040888963407ULL);
        v.push_back(x);
    }
    std::sort(v.begin(), v.end());
    printf("%lld\n%lld\n", v[0], v[n - 1]);
    return 0;
}
