# 12. 外部函数接口（FFI）

Rlyeh 通过 `extern` 声明调用外部 C ABI 函数（编译器生成符合 C 调用约定的桩）。

## 12.1 声明与调用

```rlyeh
extern "C" {
    fn abs(x: i64) -> i64;
}

fn main() {
    let v = abs(-42);              // 42
}
```

混合构建：`.rl` 源文件与 C 文件可同工程混合编译，编译器按扩展名路由到各自后端（Rlyeh→LLVM、C→clang），统一链接。

## 12.2 链接 C 库

在 `Rlyeh.toml` 中声明依赖的链接库（如 `libc`），构建时注入链接参数。详见 `examples/` 下的 FFI 示例（`examples/ffi-*.rl`、`examples/ffi/*.c`）。

> 当前为低阶 FFI：直接声明 C 函数签名，与 C 侧人工对齐 ABI（参数/返回类型、调用约定）。高阶绑定（自动类型映射、安全封装）规划中。

---

[← 上一章：编译目标与工具链](./11-targets-toolchain.md) | [返回指南目录](./index.md) | [下一章：参考与已知限制 →](./13-references-limits.md)
