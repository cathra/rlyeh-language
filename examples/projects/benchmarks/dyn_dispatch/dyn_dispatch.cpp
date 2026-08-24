// 基准: dyn_dispatch —— 2000 万次虚函数多态分派
// 与 dyn_dispatch.zeta 逻辑严格一致。输出 = 70000000
#include <cstdio>
#include <cstdint>

struct Shape {
    virtual int64_t sides() const = 0;
    virtual ~Shape() {}
};
struct Tri : Shape {
    int64_t n;
    explicit Tri(int64_t v) : n(v) {}
    int64_t sides() const override { return n + 3; }
};
struct Quad : Shape {
    int64_t n;
    explicit Quad(int64_t v) : n(v) {}
    int64_t sides() const override { return n + 4; }
};

int main() {
    Tri t{0};
    Quad q{0};
    Shape *d1 = &t;
    Shape *d2 = &q;
    int64_t sum = 0;
    for (int64_t i = 0; i < 10000000; i++) {
        sum += d1->sides();
        sum += d2->sides();
    }
    printf("%lld\n", sum);
    return 0;
}
