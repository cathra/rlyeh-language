# 7. 模块与可见性

> 速查 `module` / `pub` / `import` 语法与限制。讲解见 [指南 §7 模块系统](../guide/07-modules.md)。

---

## 7.1 定义与导入

```rlyeh
module math {
    pub const PI: f64 = 3.14159;          // 对外可见
    pub fn square(x: i64) -> i64 { x * x }
    fn secret() -> i64 { 42 }             // 私有（模块外不可见）
}
import math::PI;                          // 原名导入
import math::square as sq;                // 导入 + 重命名
println(PI);                              // 3.141590
println(square(5));                       // 25
```

- 顶层 `module` 声明；`pub` 标记对外可见，**默认私有**。
- 别名导入 + 重命名：`import 模块::符号 as 别名`。
- 跨模块路径：`模块名::Enum::Variant` / `模块名::CONST`。

> **C 对照**：`module` ≈ `.c` + 对应 `.h` 合体；`pub` ≈ 头文件里非 `static` 的符号；`import` ≈ 精准版 `#include`（只取需要的符号，不灌入全局）。

---

## 7.2 多文件模块

`module foo;` 的实现在以下位置（扁平名字空间，路径即 `模块名::`）：

```
foo.rl            ← 或
foo/module.rl
```

- 模块内所有文件的 `pub` 项挂在同一模块名下，互相可直接用对方符号，不需互相 `import`。

---

## 7.3 当前模块能力与限制

| 能力 | 状态 |
|------|------|
| 嵌套模块 | ✅ 内联 + 多文件嵌套 |
| `pub use` 重导出 / 组导入 / glob | ✅ 已支持 |
| 别名导入 `as` | ✅ |
| 相对 `super::` / `crate::` | 按扁平名字空间决策排除；使用 `模块名::` 绝对路径 |
| 默认私有 + `pub` 校验 | 🟢 灰度（`--visibility`，默认 `off`；std 跨模块 `pub` 迁移暂缓） |

---

## 更多示例

多文件模块（完整可运行示例见 [`examples/by-chapter/07-modules/`](../../examples/by-chapter/07-modules/)）：

```rlyeh
// geometry.rl
module geometry {
    pub struct Point { x: f64, y: f64 }
    pub fn dist(a: Point, b: Point) -> f64 { /* ... */ }
}
// main.rl
import geometry::dist as distance;
fn main() { println(distance(p1, p2)); }
```

---

[← 上一章：函数与闭包](./06-functions-closures.md) | [返回手册目录](./index.md) | [下一章：泛型与单态化 →](./08-generics.md)
