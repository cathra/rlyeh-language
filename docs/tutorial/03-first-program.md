# 3. 第一个程序

> 本章目标：逐行读懂一个完整可运行的 Rlyeh 程序，理解"编译 → 链接 → 运行"的模型，并对照 C 建立直觉。**边读边动手**效果最好：把代码存成 `hello-world.rl`，跟着命令跑一遍。

---

## 3.1 最小程序

```rlyeh
// hello-world.rl
fn main() {
    println("Hello, Rlyeh!");
}
```

编译与运行：

```bash
rlyeh build hello-world.rl     # 生成可执行文件 hello-world
./hello-world                  # 运行

# 或者一步到位：编译并立即运行
rlyeh run hello-world.rl
```

输出：

```
Hello, Rlyeh!
```

---

## 3.2 逐行拆解（对照 C）

### `fn main()` —— 程序入口

```rlyeh
fn main() {
```

| Rlyeh | C 等价 | 说明 |
|-------|--------|------|
| `fn` | 函数返回类型前的声明符 | Rlyeh 用 `fn` 关键字声明函数（而非 C 的"返回类型 名字"） |
| `main()` | `int main(void)` / `int main()` | 程序入口，名字叫 `main` |
| 无返回类型 | `int main` | Rlyeh 中 `main` 默认无返回值（等价于返回 `()`，类似 C 的 `void`） |

> **C 程序员的视角**：Rlyeh 的 `fn main() { }` 大致等价于 C 的 `void main(void) { }`。Rlyeh 没有"main 必须返回 int 表示退出码"的强约束（MVP 简化）。

### `println("Hello, Rlyeh!")` —— 打印

```rlyeh
    println("Hello, Rlyeh!");
```

- `println` 是内建打印宏，自动在末尾加换行（类似 C 的 `printf("...\n")`）。
- 字符串 `"Hello, Rlyeh!"` 是**字符串字面量**。
- 语句以**换行或分号**结束。Rlyeh 比 C 灵活：这里的 `println(...)` 后面有没有分号都可以（块内最后一条表达式作为返回值时例外，详见 §3.4）。

对照 C：

```c
#include <stdio.h>
int main(void) {
    printf("Hello, Rlyeh!\n");   // 要自己写 \n
    return 0;
}
```

### `}` —— 结束

Rlyeh 用大括号 `{}` 划分块，和 C 一样。但 Rlyeh **强制缩进风格**（类似 Python 的体感、Go 的 `gofmt`）：代码块靠缩进来组织可读性，编译器对格式有要求，`rlyeh fmt` 可以自动帮你排版。

---

## 3.3 打印多个值：不用格式字符串

C 的 `printf` 要靠 `%d`/`%s` 占位符，类型写错就崩。Rlyeh 的 `println` 直接逗号分隔多个值，自动按类型格式化：

```rlyeh
fn main() {
    let name = "world";
    let count = 3;
    println("Hello, ", name, "! count = ", count);
}
```

输出：

```
Hello, world! count = 3
```

> **为什么更省心**：你不用记 `%d`/`%ld`/`%f`/`%s` 的区别，也不用担心占位符和参数类型不匹配（C 里这是经典的未定义行为来源）。

### 占位符 `{}` 写法（可选）

如果你想完全控制格式，也可以用和 Rust 类似的 `{}` 占位：

```rlyeh
println("Hello, {}! count = {}", "world", 3);   // Hello, world! count = 3
```

`{}` 是个值占位符，按顺序填入后面的参数。这种写法在循环里拼接字符串时很常用（见 [指南 §10 标准库](../guide/10-stdlib.md) 的 `String`）。

---

## 3.4 理解"表达式 vs 语句"：一个 C 没有的小区别

Rlyeh 有个和 C 不同、但很重要且好用的规则：**块里最后一条表达式的值，就是这个块（和包含它的函数）的返回值**——不需要写 `return`。

```rlyeh
fn add(a: i64, b: i64) -> i64 {
    a + b          // 没有分号 → 这是"尾表达式"，就是返回值
}
```

对比 C：

```c
long add(long a, long b) {
    return a + b;   // C 必须显式 return
}
```

- 行尾**有分号** `a + b;`：表示"这是一个语句，丢弃结果"。
- 行尾**无分号** `a + b`：表示"这是表达式，它的值被返回"。

> **C 程序员的视角**：这有点像 C 里"三元表达式 `cond ? x : y` 能作为值"，但 Rlyeh 把整段函数体都当成可以产出值的表达式。刚开始可能不习惯，但它让代码更简洁，尤其是配合 `match`/`if` 表达式（见 [指南 §3](../guide/03-basic-syntax.md)）。
>
> 初学建议：**函数体最后一行如果不想返回，就加分号**。等熟悉了再玩"尾表达式返回"。

---

## 3.5 编译模型：Rlyeh 怎么变成可执行文件

理解编译流程，有助于你调试和排查问题：

```
hello-world.rl
   │  rlyeh build
   ▼
[ 词法/语法分析 ]  →  [ 类型检查 ]  →  [ 中间表示 HIR/MIR/LIR ]  →  [ LLVM IR ]
                                                                         │
                                                                         ▼
                                                               [ 汇编 + 链接 ]  →  hello-world
```

- Rlyeh 前端（词法、语法、类型检查）用 Rust 写，但**这和你无关**——你只写 `.rl` 文件。
- 后端走 **LLVM**（和 Clang 一样），所以生成的机器码质量和 C/Clang 一个级别。
- 最终链接出**独立的原生可执行文件**，不依赖 Rlyeh 运行时（轻量内置运行时除外），部署方式和 C 程序一样：`scp` 过去、`chmod +x`、直接跑。

> **C 程序员的视角**：`rlyeh build` ≈ `clang hello-world.rl -o hello-world`。区别是 Rlyeh 在链接前多了"所有权/借用检查"这一步——这也是它比 C 安全的原因。

---

## 3.6 常见新手问题

| 现象 | 原因 | 解决 |
|------|------|------|
| `error: cannot find function println` | 拼写错误（注意小写 `println`） | 检查拼写 |
| `error: type mismatch` | 给 `println` 传了它不支持的类型 | 先用 `i64`/`f64`/`String`/`bool` 等基本类型 |
| 改了代码没生效 | 跑的是旧的 `./hello-world`，没重新 `build` | 用 `rlyeh run hello-world.rl` 一步到位 |
| 报错指向奇怪的位置 | 大括号/缩进不匹配 | 跑 `rlyeh fmt hello-world.rl -w` 自动排版 |

---

## 3.7 动手练习

1. 把 `"Hello, Rlyeh!"` 改成你的名字，重新运行。
2. 打印三个数相加的结果：`println(1 + 2 + 3)`。
3. 写一个 `add` 函数（见 §3.4），在 `main` 里调用并打印结果。

完成后，你已经具备"写 → 编 → 跑"的闭环能力。下一章我们用 `rlyeh new` 建立一个真正的项目结构。

---

[← 上一章：工具链部署与安装](./02-install.md) | [返回教程目录](./index.md) | [下一章：实战：创建并发布项目 →](./04-publish.md)
