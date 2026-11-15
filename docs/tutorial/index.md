# Rlyeh 教程指南（新手入门）

> 最后更新：2026-11-15
>
> **定位**：本文带你完成 Rlyeh 的**安装 → 第一个程序 → 发布项目**全流程，适合第一次接触 Rlyeh 的读者（**尤其面向有 C 语言基础、Rust/Rlyeh 零基础的读者**，每章都配有"与 C 对照"的讲解）。
> **阅读约定**：文中用 `> **C 程序员的视角**：…` 标注的引用块，专门帮你把新概念和 C 里的熟悉事物建立联系；用 `> **注意**` / `> **为什么**` 标注的，是容易踩坑或值得理解原理的地方。
> **学语言**：渐进式语言教程见 [guide](../guide/index.md)（完整、示例可运行）；语言速查与词法细节见 [manual](../manual/index.md)。
> **权威规范**：[grammar.md](../grammar.md)（EBNF）/ [semantics.md](../semantics.md) / [memory-model.md](../memory-model.md) / [actor-model.md](../actor-model.md) / [std-lib.md](../std-lib.md)。

---

## 章节导航

1. [认识 Rlyeh](./01-what-is-rlyeh.md)
2. [工具链部署与安装](./02-install.md)
3. [第一个程序](./03-first-program.md)
4. [实战：创建并发布项目](./04-publish.md)

---

## 0.2.0 新能力速览

当前版本 **0.2.0（自举能力补齐）** 在 MVP 之上新增：

- 泛型 protocol/impl、嵌套模块、`pub use`/组导入/glob、`#[derive(Clone/PartialEq/Debug)]`
- `unsafe` 块 / 裸指针 / `#[repr(C)]`、`extern "C"` FFI、`dyn Protocol` 含 `Self` 与 `Any` 下转换
- 跨函数边界闭包 / `move` / `'static`、Actor / async / 原子类型 / 线程等并发地基
- 元组值/解构、`if let` / `while let`、`match` 守卫 + 范围/或模式、`Drop`/RAII
- `const` / `static`、`mem::swap/replace/take`、`?` 经 `From` 转换、`try { .. }` 错误聚合块
- 优化级别 `-O0/-O1/-O2/-O3/-Os`（默认 O3）与 `--no-heap` 无堆编译模式

语言参考见 [手册](../manual/index.md)，编译器/优化级别见 [§13 工具链](../manual/13-toolchain.md)。

## 下一步

- 完整学习语言 → [编程语言指南](../guide/index.md)
- 语言速查 / 词法细节 / 命令参考 → [语言手册](../manual/index.md)
- 工具链构建与故障排查 → [../../toolchains/README.md](../../toolchains/README.md)、[../../toolchains/docs/REBUILD.md](../../toolchains/docs/REBUILD.md)
- 跨语言性能对比 → [../../examples/projects/benchmarks/README.md](../../examples/projects/benchmarks/README.md)
