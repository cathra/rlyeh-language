// 基准: dyn_dispatch —— 2000 万次 vtable 多态分派（函数指针表）
// 与 dyn_dispatch.zeta 逻辑严格一致。输出 = 70000000
#include <stdio.h>
#include <stdint.h>

typedef struct {
    int64_t (*sides)(const void *self);
} Vtbl;

typedef struct { const Vtbl *vt; int64_t n; } TriObj;
typedef struct { const Vtbl *vt; int64_t n; } QuadObj;

static int64_t tri_sides(const void *self) {
    return ((const TriObj *)self)->n + 3;
}
static int64_t quad_sides(const void *self) {
    return ((const QuadObj *)self)->n + 4;
}

static const Vtbl tri_vt = { tri_sides };
static const Vtbl quad_vt = { quad_sides };

int main(void) {
    TriObj t = { &tri_vt, 0 };
    QuadObj q = { &quad_vt, 0 };
    const void *d1 = &t;
    const void *d2 = &q;
    int64_t sum = 0;
    for (int64_t i = 0; i < 10000000; i++) {
        const Vtbl *vt = *(const Vtbl *const *)d1;
        sum += vt->sides(d1);
        vt = *(const Vtbl *const *)d2;
        sum += vt->sides(d2);
    }
    printf("%lld\n", sum);
    return 0;
}
