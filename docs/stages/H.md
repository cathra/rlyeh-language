# 阶段 H — 一等函数（闭包 + 函数指针）

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-*.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


> 现状：函数类型与函数指针、无捕获闭包、捕获闭包（IIFE MVP）、闭包值对象、`dyn Trait` 均已实现（H1–H5 ✅）。各任务实现细节、执行情况与技术细节见任务树 [`stage-g-l.md`](../tasks/stage-g-l.md) 对应叶子文档（`h1`–`h5`）。按引用捕获仍规划中。

| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| H1 | **函数类型与函数指针**：`fn(A) -> B` 类型（含泛型实例化）、函数作为值传递与调用（codegen 函数指针） | ✅ | [`h1-fn-pointer.md`](../tasks/leaf/h1-fn-pointer.md) |
| H2 | **无捕获闭包**：` | x | [`h2-closure.md`](../tasks/leaf/h2-closure.md) |
| H3 | **捕获闭包**：按值捕获 desugar 为匿名结构体（捕获字段 + `call` 方法）；`move` 语义；按引用捕获依赖 G | ✅（IIFE MVP；执行情况见 [`h3-capture-closure.md`](tasks/leaf/h3-capture-closure.md | [`h3-capture-closure.md`](../tasks/leaf/h3-capture-closure.md) |
| H4 | **`dyn Trait`**：trait 对象（数据指针 + vtable：drop/size/align/方法表），`dyn Trait` 类型 + 强制转换 | ✅（MVP；执行情况见 [`h4-dyn-trait.md`](tasks/leaf/h4-dyn-trait.md | [`h4-dyn-trait.md`](../tasks/leaf/h4-dyn-trait.md) |
| H5 | **闭包值对象**：`let f = \ | x: i64\ | [`h5-closure-value.md`](../tasks/leaf/h5-closure-value.md) |
