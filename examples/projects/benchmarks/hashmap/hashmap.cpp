// benchmark: 20 万 insert + 20 万 get（LCG 键）—— 与 hashmap.zeta 同逻辑
#include <cstdio>
#include <unordered_map>

int main() {
    std::unordered_map<long long, long long> m;
    m.reserve(400000);

    long long x = 12345;
    for (int i = 0; i < 200000; i++) {
        x = (long long)((unsigned long long)x * 6364136223846793005ULL + 1442695040888963407ULL);
        m[x] = i;
    }

    long long sum = 0;
    long long y = 12345;
    for (int j = 0; j < 200000; j++) {
        y = (long long)((unsigned long long)y * 6364136223846793005ULL + 1442695040888963407ULL);
        auto it = m.find(y);
        if (it != m.end()) sum += it->second; else sum += 1;
    }
    printf("%lld\n", sum);
    return 0;
}
