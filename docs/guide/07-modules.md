# 7. 模块系统

```rlyeh
module math {
    pub const PI: f64 = 3.14159;
    pub fn square(x: i64) -> i64 { x * x }
}
import math::PI;                 // 别名导入
import math::square as sq;

println(PI);                    // 3.141590
println(square(5));             // 25
```

- 多文件模块：`module foo;` → `foo.rl` / `foo/module.rl`（扁平名字空间，路径即「模块名::」）。
- 跨模块路径：`模块名::Enum::Variant` / `模块名::CONST`。
- 不支持 `pub use`（重导出）、相对 `super::`、嵌套模块声明（规划中）。

---

[← 上一章：数组与索引](./06-arrays-slices.md) | [返回指南目录](./index.md) | [下一章：内存管理 →](./08-memory.md)
