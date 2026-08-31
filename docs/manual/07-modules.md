# 7. 模块与可见性

```rlyeh
module math {
    pub const PI: f64 = 3.14159;
    pub fn square(x: i64) -> i64 { x * x }
}
import math::PI;                 // 别名导入
import math::square as sq;       // 导入 + 重命名
```

- 顶层 `module` 声明；`pub` 可见性（默认私有）
- 别名导入 + 重命名；跨模块路径：`模块名::Enum::Variant` / `模块名::CONST`
- 多文件模块：`module foo;` → `foo.rl` / `foo/module.rl`（扁平名字空间，路径即「模块名::」）
- **不支持**（规划中）：`pub use` 重导出、相对 `super::`、嵌套模块声明

> 模块路径保留 `::` 分隔；模块名须与目录/文件名对齐（“`` 路径即模块名:: ``”）。

---

[← 上一章：函数与闭包](./06-functions-closures.md) | [返回手册目录](./index.md) | [下一章：泛型与单态化 →](./08-generics.md)
