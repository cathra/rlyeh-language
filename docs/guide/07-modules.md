# 7. 模块系统

> 本章目标：学会把代码组织成**模块（module）**，以及如何在模块间**导入（import）** 符号。模块让你从"单文件脚本"过渡到"多文件工程"，是写可维护项目的基础。

---

## 7.1 为什么需要模块

在 C 里，你用 `.h` 头文件声明接口、`.c` 文件实现，靠 `#include` 拼到一起。Rlyeh 用更内聚的 `module` 概念取代这套机制：

> **C 程序员的视角**：
> - `module` ≈ 一个 `.c` + 对应 `.h` 的**合体**——在一个地方同时声明"对外暴露什么"。
> - `pub` ≈ 头文件里声明、且不属于 `static` 的函数/变量（即"对外可见"）。
> - `import` ≈ `#include` + 只取需要的符号（更精准，不会把整个头文件灌进来）。

---

## 7.2 定义模块

用 `module` 关键字定义一个模块，里面用 `pub` 标记"对外可见"的成员：

```rlyeh
module math {
    pub const PI: f64 = 3.14159;
    pub fn square(x: i64) -> i64 { x * x }

    // 没有 pub 的成员对外不可见（模块私有）
    fn secret() -> i64 { 42 }
}
```

> **私有 vs 公开**：不加 `pub` 的项只在模块内部可见，外部 `import` 后也用不了。这类似 C 里 `.c` 文件的 `static` 函数——只在翻译单元内可用。Rlyeh 在编译期就禁止外部访问私有项。

---

## 7.3 导入符号 `import`

```rlyeh
import math::PI;                 // 导入 PI（名字不变）
import math::square as sq;       // 导入并改名（避免命名冲突）

println(PI);                    // 3.141590
println(square(5));             // 25
println(sq(5));                 // 25（用别名）
```

- `import 模块::符号`：按原名导入。
- `import 模块::符号 as 别名`：导入后改名（类似 C 没有、但 Rust/Python 都有的 `import ... as`）。

> **C 程序员的视角**：`import math::square as sq` 大致等价于 C 里 `/* 假设有命名空间 */ using math::square;` 再 `#define sq math::square;`。Rlyeh 的 `import` 是**显式、按需**的：你 import 什么才能用什么，不会有 `#include` 那种"把一整个头文件几千个符号灌进全局"的副作用。

---

## 7.4 跨模块路径

导入后，也可以用"完整路径"直接引用，不必先 `import`：

```rlyeh
println(math::PI);                    // 直接用 模块::常量
let s = math::square(5);              // 模块::函数

// 跨模块引用枚举变体 / 常量
let v = other_mod::Status::Ready;     // 模块名::Enum::Variant
let max = other_mod::MAX_SIZE;        // 模块名::CONST
```

路径规则：**模块名::** 作为前缀，后面跟 `Enum::Variant`、常量、函数等。

---

## 7.5 多文件模块

一个模块可以拆到多个文件。声明 `module foo;` 时，编译器会在以下位置寻找实现：

```
foo.rl            ← foo/module.rl 或 foo.rl 二选一（扁平名字空间）
foo/module.rl
```

- 模块内是**扁平名字空间**：所有文件里的 `pub` 项都挂在同一模块名下（路径即 `模块名::`）。
- 这意味着同一个模块的不同文件之间可以直接用对方定义的 `pub` 符号，不需要互相 `import`。

> **C 程序员的视角**：多文件模块 ≈ 多个 `.c` 文件编进同一个"翻译单元组"，共享一个对外接口。你不用在每文件里重复 `#include` 自己的头。

---

## 7.6 当前 MVP 限制（规划中特性）

以下功能 MVP **尚未实现**，编写时规避：

| 特性 | 说明 |
|------|------|
| `pub use`（重导出） | 不能把导入的符号再作为本模块公开 API 转手 |
| 相对路径 `super::` | 不能用 `super` 引用父模块，暂时都用绝对 `模块::` 路径 |
| 嵌套模块声明 | 不能在文件内写 `module a { module b { } }` 嵌套，模块在顶层声明 |

> 这些限制不影响大多数中小项目——你仍可以用"多个顶层 `module` + `import`"组织代码。

---

## 7.7 综合示例：拆分一个工程

```rlyeh
// 文件 geometry.rl
module geometry {
    pub struct Point { x: f64, y: f64 }
    pub fn dist(a: Point, b: Point) -> f64 {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        // 此处简化，实际应 (dx*dx + dy*dy) 开方；示例使用平方距离示意
        dx * dx + dy * dy
    }
}

// 文件 main.rl
import geometry::Point;
import geometry::dist as distance;

fn main() {
    let a = Point { x: 0, y: 0 };
    let b = Point { x: 3, y: 4 };
    println(distance(a, b));        // 25（平方距离）
}
```

---

## 练习

1. 按 [`examples/by-chapter/07-modules/`](../../examples/by-chapter/07-modules/) 下的 `geometry.rl` + `main.rl` 结构，自己拆一个 `math` 模块并在 `main` 里 `import`。
2. 给模块里的 `secret()` 去掉 `pub`，在外部 `import` 它，看编译器是否报错（理解私有性在编译期强制）。
3. 用 `import math::square as sq` 改名导入，避免与本地同名函数冲突。

---

[← 上一章：数组与索引](./06-arrays-slices.md) | [返回指南目录](./index.md) | [下一章：内存管理 →](./08-memory.md)
