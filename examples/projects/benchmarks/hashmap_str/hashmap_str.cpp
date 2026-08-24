// 基准: hashmap_str —— 1 万条字符串键哈希表插入与查询
// 与 hashmap_str.zeta 逻辑严格一致。输出 = 49995000
#include <cstdio>
#include <cstdint>
#include <string>
#include <unordered_map>

int main() {
    const int n = 10000;
    std::unordered_map<std::string, int64_t> m;
    m.reserve(n * 2);
    for (int i = 0; i < n; i++) {
        m.emplace("key_" + std::to_string(i), i);
    }
    int64_t sum = 0;
    for (int i = 0; i < n; i++) {
        auto it = m.find("key_" + std::to_string(i));
        if (it != m.end()) sum += it->second;
        else sum -= 1;
    }
    printf("%lld\n", sum);
    return 0;
}
