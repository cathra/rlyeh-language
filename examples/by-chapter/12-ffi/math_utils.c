// 对应 docs/guide/12-ffi.md —— 外部函数接口（C 侧）
// 与 main.rl 混合编译：rlyeh build 12-ffi/main.rl 12-ffi/math_utils.c -o app
long add(long a, long b) {
    return a + b;
}

long multiply(long a, long b) {
    return a * b;
}
