// benchmark: 256x256 f64 矩阵乘法 —— 与 matmul.zeta 同逻辑
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    const int n = 256;
    const int total = n * n;
    double *a = malloc(total * sizeof(double));
    double *b = malloc(total * sizeof(double));
    double *c = malloc(total * sizeof(double));
    for (int i = 0; i < total; i++) { a[i] = 1.0001; b[i] = 1.0001; c[i] = 0.0; }
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
    free(a); free(b); free(c);
    return 0;
}
