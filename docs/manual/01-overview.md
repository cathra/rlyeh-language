# 1. 语言概述（手册）

> 本文是**语言手册**的入口。手册定位为**权威速查**：每个语法/类型/运算符/命令的精确形态都在这里。需要"为什么 + 循序渐进讲解"请看 [编程语言指南](../guide/index.md)；需要"从零安装到发布"看 [教程](../tutorial/index.md)。
>
> **读者背景假设**：本文档的旁注 `> **C 对照**` 专门帮有 C 基础、无 Rust/Rlyeh 经验的读者建立直觉。

---

## 1.1 一句话定位

Rlyeh 是一门**系统级编程语言**，编译为原生机器码。设计公式：

> **Rlyeh = Rust 的安全性 + Go 的编译速度 + Erlang 的并发模型 + 数学的自然表达。**

| 目标 | 对标 | 落地形态 |
|------|------|----------|
| 内存安全、零 GC | Rust | 分层内存模型（L0 所有权 → L1 区域 → L2 Rc → L3 GC） |
| 编译速度极快 | Go | 模块级增量编译 |
| 并发模型一等公民 | Erlang / Akka | `actor` + 运行时监督 |
| 数学式语法直觉 | Python / MATLAB | 比较链 `0 < x < 10`、集合判断 `x in (1, 3, 5)` |

> **C 对照**：Rlyeh 默认**零运行时开销**（不是 Java/Go 那种有后台 GC 的语言）。安全来自编译期分析，不来自运行时托管。所以从性能心智模型上，它和 C 是同一档。

---

## 1.2 与 C 的能力对照速查

| 你要做的事 | C 怎么干 | Rlyeh 怎么干 |
|------------|----------|--------------|
| 写入口 | `int main()` | `fn main()` |
| 打印 | `printf("x=%d\n", x)` | `println("x=", x)` |
| 变量 | `int x = 6;`（默认可改） | `let x = 6;`（默认不可改，`let mut` 才可改） |
| 堆分配 | `malloc`/`free` | 所有权自动释放 / `region` / `Box`/`Rc`/`Gc` |
| 结构体 | `struct { int x; };` | `struct { x: i64 }` |
| 枚举 | 整数常量 | 可携带数据的 `enum` + `match` |
| 循环 | `for`/`while` | `for`/`while`/`loop` + 数值区间 |
| 函数指针 | `int (*fp)(int)` | `fn(i64) -> i64`（一等值） |
| 并发 | `pthread` + 锁 | `actor` + 消息 |
| 调 C 库 | 直接链接 | `extern "C"` |

---

## 1.3 MVP 已落地核心能力

- 标量 / 聚合类型、引用与借用（严格借用检查）、`ref`/`ref mut` 模式
- `struct` / `enum`（具名变体 + 显式判别式）/ `trait` + `impl` + 泛型单态化
- 类型联合 `A | B`、枚举显式判别式（`U1`–`U3`）
- 切片引用 `&[T]` / `&mut [T]`（`S1`–`S3`）、动态切片、`as` 数值转换（`U6`）
- 闭包（H1 函数指针 / H2 无捕获 / H3 捕获 IIFE / H5 闭包值对象）、`dyn Trait`（H4）
- 分层内存（L0 所有权 → L1 区域 → L2 `Rc`/`Arc` → L3 `Gc`）
- Actor 并发（监督 / 交叉编译 WASM，`L4`）、`async`/`.await`（S1/W1–W5）
- 标准库：String / Vec / HashMap / Option / Result / 时间 / IO / 网络 / 同步 / 序列化（JSON+TOML）/ 迭代器

> **MVP 含义**：本文档（手册 + 指南）**只描述上述已实现项**。规划中特性会在各章明确标注，不会引导你使用不存在的功能。完整已知限制见 [§14 限制](./14-limits.md) 与 [指南 §13](../guide/13-references-limits.md)。

---

## 1.4 手册阅读地图

| 你想查 | 去 |
|--------|----|
| 关键字 / 字面量 / 运算符字符 | [§2 词法与字面量](./02-lexical.md) |
| 所有类型（标量/聚合/引用/智能指针） | [§3 类型系统](./03-types.md) |
| 运算符优先级与表达式 | [§4 表达式与运算符](./04-expressions-operators.md) |
| 语句与控制流 | [§5 语句与控制流](./05-statements-control-flow.md) |
| 函数 / 闭包 / 函数指针 | [§6 函数与闭包](./06-functions-closures.md) |
| 模块 / 导入 | [§7 模块](./07-modules.md) |
| 泛型 / trait | [§8 泛型](./08-generics.md) |
| 内存分层模型 | [§9 内存](./09-memory.md) |
| Actor / async | [§10 并发](./10-concurrency.md) |
| 标准库总览 | [§11 标准库](./11-stdlib.md) |
| 编译器构建 | [§12 编译器构建](./12-compiler-build.md) |
| 工具链命令 | [§13 工具链](./13-toolchain.md) |
| 已知限制 | [§14 限制](./14-limits.md) |
| 每个标准库类型的完整成员 | [std/ 子目录](./std/index.md) |

---

## 更多示例

对比 C 与 Rlyeh 的入口，感受语法差异：

```c
// C
#include <stdio.h>
int main() { printf("x=%d\n", 6); return 0; }
```

```rlyeh
// Rlyeh
fn main() {
    println("x=", 6);     // 自动按类型格式化，无占位符、无 %d 陷阱
}
```

可运行版本见 [`examples/by-chapter/01-what-is-rlyeh.rl`](../../examples/by-chapter/01-what-is-rlyeh.rl)。

---

[← 返回手册目录](./index.md) | [下一章：词法与字面量 →](./02-lexical.md)
