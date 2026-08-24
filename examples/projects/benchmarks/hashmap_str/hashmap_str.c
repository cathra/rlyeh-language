// 基准: hashmap_str —— 1 万条字符串键哈希表插入与查询
// 与 hashmap_str.zeta 逻辑严格一致。输出 = 49995000
// 实现: 线性探测 + djb2 哈希 + strdup 键（每次构造键 = 堆分配，与 Zeta format! 对称）
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

#define N 10000
#define CAP 16384

typedef struct {
    const char *key;
    int64_t val;
    int used;
} Entry;

static uint64_t djb2(const char *s) {
    uint64_t h = 5381;
    while (*s) h = h * 33 + (unsigned char)*s++;
    return h;
}

static void insert(Entry *t, const char *k, int64_t v) {
    uint64_t idx = djb2(k) & (CAP - 1);
    while (t[idx].used) idx = (idx + 1) & (CAP - 1);
    t[idx].key = k;
    t[idx].val = v;
    t[idx].used = 1;
}

static int64_t get(Entry *t, const char *k) {
    uint64_t idx = djb2(k) & (CAP - 1);
    while (t[idx].used) {
        if (strcmp(t[idx].key, k) == 0) return t[idx].val;
        idx = (idx + 1) & (CAP - 1);
    }
    return -1;
}

int main(void) {
    Entry *t = calloc(CAP, sizeof(Entry));
    char buf[32];
    for (int i = 0; i < N; i++) {
        snprintf(buf, sizeof buf, "key_%d", i);
        insert(t, strdup(buf), i);
    }
    int64_t sum = 0;
    for (int i = 0; i < N; i++) {
        snprintf(buf, sizeof buf, "key_%d", i);
        int64_t v = get(t, strdup(buf));
        if (v >= 0) sum += v;
        else sum -= 1;
    }
    printf("%lld\n", sum);
    return 0;
}
