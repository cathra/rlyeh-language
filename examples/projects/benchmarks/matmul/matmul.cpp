// benchmark: 256x256 f64 矩阵乘法 —— 与 matmul.zeta 同逻辑
#include <cstdio>
#include <vector>

int main() {
    const int n = 256;
    const int total = n * n;
    std::vector<double> a(total, 1.0001), b(total, 1.0001), c(total, 0.0);
    for (int x = 0; x < n; x++) {
        for (int y = 0; y < n; y++) {
            double s = 0.0;
            for (int k = 0; k < n; k++) s += a[x * n + k] * b[k * n + y];
            c[x * n + y] = s;
        }
    }
    double sum = 0.0;
    for (int i = 0; i < total; i++) sum += c[i];
    printf("%f\n", sum);
    return 0;
}
