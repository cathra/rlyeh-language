# 12. 外部函数接口（FFI）

> 本章目标：学会让 Rlyeh 代码**调用 C 写的函数**（反之亦然）。对 C 程序员来说，FFI 是"渐进式采用 Rlyeh"的桥梁——你可以把现有 C 库直接拿来用，不用重写。
>
> 本章是 **C 程序员最该读懂的一章**：Rlyeh 的 FFI 几乎就是 C 调用约定的直接映射。

---

## 12.1 为什么需要 FFI

- **复用现有 C 生态**：你公司几十年的 C 库、OpenSSL、SQLite、系统 API，不必用 Rlyeh 重写。
- **渐进式迁移**：新模块用 Rlyeh 写，老模块留 C，两边互相调用。
- **访问系统底层**：某些 OS 接口只有 C ABI。

> **C 程序员的视角**：FFI 的本质就是"让 Rlyeh 生成符合 **C 调用约定（calling convention）** 的函数调用桩"。你 `extern "C"` 声明一个 C 函数签名，Rlyeh 就按 C 的栈布局传参——和你在 C 里 `dlopen`/`dlsym` 拿到函数指针调用是同一回事，只是编译器帮你把"签名对齐"做了。

---

## 12.2 声明与调用 C 函数

用 `extern "C"` 块声明外部函数，之后像调用普通 Rlyeh 函数一样调用：

```rlyeh
extern "C" {
    fn abs(x: i64) -> i64;          // 声明：来自 C 标准库的 abs
    fn my_c_func(a: i64, b: i64) -> i64;
}

fn main() {
    let v = abs(-42);              // 42：直接调用，编译器生成 C ABI 调用
    let r = my_c_func(3, 4);       // 调用你自己的 C 函数
}
```

- `extern "C"`：告诉编译器"这些函数遵循 C 调用约定"。
- 你**只写签名、不写实现**——实现在链接阶段由 C 侧提供。
- 调用方式和普通 Rlyeh 函数无差别（编译器在背后生成正确的栈帧/寄存器传参）。

---

## 12.3 混合构建（.rl 与 .c 同工程）

Rlyeh 支持 `.rl` 源文件与 `.c` 文件在**同一工程里混合编译**：编译器按扩展名把 `.rl` 路由到 Rlyeh 前端、`.c` 路由到 clang，最后统一链接成单一可执行文件。

```c
// math_utils.c
long add(long a, long b) {
    return a + b;
}
```

```rlyeh
// main.rl
extern "C" {
    fn add(a: i64, b: i64) -> i64;
}

fn main() {
    println(add(2, 3));    // 5：调用 C 实现的 add
}
```

构建（以仓库 `examples/ffi/` 下的示例为准）：

```bash
rlyeh build main.rl math_utils.c -o app
./app
```

> **C 程序员的视角**：这等价于 `clang main-ffi-stub.o math_utils.c -o app`——只是 Rlyeh 把"编 Rlyeh 部分"和"编 C 部分"串起来了。你熟悉的 `clang`/`ld` 链接规则依然适用（符号名、库搜索路径等）。

---

## 12.4 链接 C 库

要在 `Rlyeh.toml` 里声明要链接的库（如系统 `libc`、第三方 `.a`/`.so`），构建时编译器会注入对应的链接参数。具体写法见仓库 `examples/ffi-*.rl`、`examples/ffi/*.c` 下的可运行示例。

```toml
# Rlyeh.toml 片段（示意）
[dependencies.c]
libs = ["m"]          # 链接 libm（数学库），等价于 clang 的 -lm
```

> 这部分会随工具链成熟而简化；当前以示例目录为准。

---

## 12.5 类型映射（ABI 对齐要点）

跨 FFI 边界时，**两边的类型必须在内存布局上对齐**，否则会静默出错。Rlyeh 的标量类型与 C 直接对应：

| Rlyeh 类型 | C 类型 | 说明 |
|------------|--------|------|
| `i8` | `int8_t` / `signed char` | |
| `i16` | `int16_t` | |
| `i32` | `int32_t` | |
| `i64` | `int64_t` / `long`(64位) | |
| `u8`..`u64` | `uint8_t`..`uint64_t` | |
| `f32` | `float` | |
| `f64` | `double` | |
| `bool` | `_Bool` / `uint8_t` | 注意 C 的 `int` 不能直接当 bool 传 |
| `char` | `uint32_t`（Unicode 码点） | Rlyeh char 是 32 位，与 C `char`(8位) 不同 |
| `*const T` / `*mut T` | `const T*` / `T*` | 裸指针互视（G3 ✅） |

> **重点坑：**
> 1. **Rlyeh 的 `char` 是 32 位 Unicode 码点**，而 C 的 `char` 是 8 位。跨边界传字符要用 `u8`/`i32` 对齐，别直接传 `char`。
> 2. **结构体布局**：Rlyeh 的 `struct` 默认与 C 的 `struct` 内存布局一致（字段顺序、对齐），但保险起见对齐敏感场景应显式确认字段顺序与填充。
> 3. **String 不是 `char*`**：Rlyeh `String` 是带长度/容量的对象，**不能直接当 `char*` 传给 C**。需要传 C 字符串时，用 `as_str()` 拿到 `&str`/裸指针，并确保 C 侧只在使用期间有效（见借用规则）。

---

## 12.6 当前限制（规划中特性）

- **低阶 FFI**：当前是"直接声明 C 函数签名 + 人工对齐 ABI"的模式。
- **高阶绑定**（自动类型映射、安全封装、自动生成 `extern` 声明）规划中。
- 调用 C 时仍需你**自己保证**：参数类型/返回类型与 C 侧完全一致、生命周期安全（不让 C 持有 Rlyeh 借出的悬垂引用）。

> **安全建议**：尽量只在 FFI 边界传递**标量、裸指针、简单结构体**。复杂的 Rlyeh 对象（String/Vec/closure）不要直接透传给 C——在边界处做转换（如先把 String 转成 `&str` 拷贝出 C 需要的 `char*`）。

---

## 练习

1. 按 [`examples/by-chapter/12-ffi/`](../../examples/by-chapter/12-ffi/) 下的 `main.rl` + `math_utils.c`，用 `rlyeh build main.rl math_utils.c` 混合编译运行。
2. 在 C 侧加一个 `multiply`，在 Rlyeh 侧 `extern "C"` 声明并调用，验证类型对齐（`i64` ↔ `long`）。
3. 思考：哪些 Rlyeh 类型**不能**直接透传给 C？为什么 `String` 不能直接当 `char*` 用？

---

[← 上一章：编译目标与工具链](./11-targets-toolchain.md) | [返回指南目录](./index.md) | [下一章：参考与已知限制 →](./13-references-limits.md)
