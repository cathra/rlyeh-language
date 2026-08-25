// benchmark: 20 万 insert + 20 万 get（LCG 键）—— 与 hashmap.rl 同逻辑
// C 无标准哈希表，手写线性探测表（2^20 槽，负载 ~19%）
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

typedef struct {
    int64_t key, val;
    int used;
} Entry;

int main(void) {
    const size_t cap = 1u << 20;
    Entry *tab = calloc(cap, sizeof(Entry));

    int64_t x = 12345;
    for (int i = 0; i < 200000; i++) {
        x = (int64_t)((uint64_t)x * 6364136223846793005ULL + 1442695040888963407ULL);
        size_t h = (uint64_t)x & (cap - 1);
        while (tab[h].used) h = (h + 1) & (cap - 1);
        tab[h].key = x; tab[h].val = i; tab[h].used = 1;
    }

    int64_t sum = 0;
    int64_t y = 12345;
    for (int j = 0; j < 200000; j++) {
        y = (int64_t)((uint64_t)y * 6364136223846793005ULL + 1442695040888963407ULL);
        size_t h = (uint64_t)y & (cap - 1);
        while (tab[h].used && tab[h].key != y) h = (h + 1) & (cap - 1);
        if (tab[h].used) sum += tab[h].val; else sum += 1;
    }
    printf("%lld\n", sum);
    free(tab);
    return 0;
}
