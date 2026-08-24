// benchmark: 字符串拼接 10 万次 —— 与 strcat.zeta 同逻辑（容量翻倍动态缓冲）
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    size_t len = 0, cap = 16;
    char *buf = malloc(cap);
    for (int i = 0; i < 100000; i++) {
        if (len + 2 > cap) {
            cap *= 2;
            buf = realloc(buf, cap);
        }
        buf[len++] = 'a';
        buf[len++] = 'b';
    }
    printf("%zu\n", len);
    free(buf);
    return 0;
}
