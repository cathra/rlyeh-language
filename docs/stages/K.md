# 阶段 K — 错误传播与所有权层级

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-*.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


> 现状：`?` 运算符、`Box<T>`、`Rc<T>`/`Arc<T>`、`Gc<T>` 均已完成（K1–K4 ✅）。各任务实现细节、执行情况与技术细节见任务树 [`stage-g-l.md`](../tasks/stage-g-l.md) 对应叶子文档（`k1`–`k4`）。

| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| K1 | **`?` 运算符**：Result/Option 早返回 desugar（`match` + `return`），要求函数返回兼容类型 | ✅ | [`k1-question.md`](../tasks/leaf/k1-question.md) |
| K2 | **`Box<T>`**：堆分配目标 API（`Box::new`；解引用依赖 G） | ✅ | [`k2-box.md`](../tasks/leaf/k2-box.md) |
| K3 | **`Rc<T>`/`Arc<T>`**：原子/非原子引用计数（memory-model.md §4 布局；`Rc::new`/`clone`/`try_unwrap`；`Deref` 特判或依赖 G） | ✅ | [`k3-rc-arc.md`](../tasks/leaf/k3-rc-arc.md) |
| K4 | **`Gc<T>`（插件式）**：标记-清除 GC + `gc_region`（memory-model.md §5；write barrier、逃逸限制） | ✅（MVP 保守标记-清除；write barrier / 增量 / 多线程规划 | [`k4-gc.md`](../tasks/leaf/k4-gc.md) |
