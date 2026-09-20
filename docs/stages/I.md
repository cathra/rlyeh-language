# 阶段 I — 宏系统与格式化

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-g-l.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


> 现状：声明式宏（含 `$x:expr` 捕获完整表达式 token 序列）、内置格式化宏、集合宏、`r#"..."#` 原始字符串均已实现（I1/I2/I3 ✅）。各任务实现细节、执行情况与技术细节见任务树 [`stage-g-l.md`](../tasks/stage-g-l.md) 对应叶子文档（`i1`–`i3`）。限制：无卫生宏（hygiene）；`Display`/`Debug` protocol 定义仍为规划 API（std-lib.md §8）。

| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| I1 | **宏调用语法区分**：词法/语法层区分 `name!`（宏调用）与 `!`（not 一元运算符）；`macro_rules!` 声明式宏（matcher → transcriber，token 流展开于 parse 后、typecheck 前） | ✅ | [`i1-declarative-macro.md`](../tasks/leaf/i1-declarative-macro.md) |
| I2 | **`Display`/`Debug` protocol + 格式化宏**：`{}` 占位符格式化引擎；`println!`/`print!`/`format!`/`dbg!` 宏；String 拼接语义（`{}` 插入 `to_string`） | ✅ | [`i2-format-macro.md`](../tasks/leaf/i2-format-macro.md) |
| I3 | **集合宏**：`vec!`/`map!`/`arr!`（`arr!` → 数组字面量；`vec!`/`map!` → `with_capacity` + push / `insert` 序列） | ✅ | [`i3-collection-macro.md`](../tasks/leaf/i3-collection-macro.md) |
