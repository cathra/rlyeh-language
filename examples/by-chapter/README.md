# examples/by-chapter —— 与文档章节对应的可运行示例

本目录下的每个 `.rl` 文件对应 [`docs/guide/`](../docs/guide/index.md) 的一章，方便你"读一章、跑一段、做一题"。
所有示例面向 **C 基础、Rlyeh 零基础**读者，使用文档中描述的 MVP 特性。

## 章节 ↔ 示例 对照表

| 文档章节 | 示例文件 | 说明 |
|----------|----------|------|
| [01 认识 Rlyeh](../../docs/guide/01-what-is-rlyeh.md) | [`01-what-is-rlyeh.rl`](./01-what-is-rlyeh.rl) | 第一个程序 |
| [02 快速上手](../../docs/guide/02-quick-start.md) | [`02-quick-start.rl`](./02-quick-start.rl) | 编译/检查/格式化命令 |
| [03 基础语法](../../docs/guide/03-basic-syntax.md) | [`03-basic-syntax.rl`](./03-basic-syntax.rl) | 变量遮蔽、函数、无捕获闭包 |
| [04 数学式条件判断](../../docs/guide/04-math-conditions.md) | [`04-math-conditions.rl`](./04-math-conditions.rl) | 比较链、集合/区间 `in`、时间字面量 |
| [05 聚合类型与泛型](../../docs/guide/05-aggregates-generics.md) | [`05-aggregates-generics.rl`](./05-aggregates-generics.rl) | struct / enum / match / trait |
| [06 数组、Vec 与切片](../../docs/guide/06-arrays-slices.md) | [`06-arrays-slices.rl`](./06-arrays-slices.rl) | 切片引用、`map` 适配器 |
| [07 模块系统](../../docs/guide/07-modules.md) | [`07-modules/`](./07-modules/)（多文件） | `geometry.rl` + `main.rl` 一起编译 |
| [08 内存管理](../../docs/guide/08-memory.md) | [`08-memory.rl`](./08-memory.rl) | 四层模型：L0 借用 / L2 Box·Rc |
| [09 Actor 并发](../../docs/guide/09-actors.md) | [`09-actors.rl`](./09-actors.rl) | ask / send / 受监督重启 |
| [10 标准库](../../docs/guide/10-stdlib.md) | [`10-stdlib.rl`](./10-stdlib.rl) | HashMap / Option |
| [11 编译目标与工具链](../../docs/guide/11-targets-toolchain.md) | [`11-targets-toolchain.rl`](./11-targets-toolchain.rl) | `--target` 跨平台编译 |
| [12 外部函数接口](../../docs/guide/12-ffi.md) | [`12-ffi/`](./12-ffi/)（混合构建） | `main.rl` + `math_utils.c` |
| [13 参考与已知限制](../../docs/guide/13-references-limits.md) | [`13-references-limits.rl`](./13-references-limits.rl) | MVP 特性集合演示 |

## 运行方式

```bash
# 单文件
rlyeh run by-chapter/03-basic-syntax.rl

# 多文件模块（07）
rlyeh run by-chapter/07-modules/main.rl by-chapter/07-modules/geometry.rl

# FFI 混合构建（12）
rlyeh build by-chapter/12-ffi/main.rl by-chapter/12-ffi/math_utils.c -o app && ./app
```
