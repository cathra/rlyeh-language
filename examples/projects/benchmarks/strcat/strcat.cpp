// benchmark: std::string += 10 万次 —— 与 strcat.rl 同逻辑
#include <cstdio>
#include <string>

int main() {
    std::string s;
    for (int i = 0; i < 100000; i++) s += "ab";
    printf("%zu\n", s.size());
    return 0;
}
