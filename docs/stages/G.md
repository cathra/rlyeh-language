# 阶段 G — 引用与借用（L0 完整化）

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-*.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


> 现状：`&T`/`&mut T` 引用、`&str` 视图、`str` 值一等类型、裸指针、生命周期标注（MVP）、严格借用检查、`ref`/`ref mut` 模式均已实现（G1–G4 ✅）。各任务的实现细节、执行情况与技术细节见任务树 [`stage-g-l.md`](../tasks/stage-g-l.md) 对应叶子文档（`g1`–`g4`）。严格生命周期验证（`'a` 绑定推断）仍规划中。

| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| G1 | **引用类型与表达式**：`&T`/`&mut T` 类型、`&x`/`&mut x` 表达式、`*` 解引用；函数参数 `x: &T`、返回 `&T`；`ref` 模式。typecheck 类型规则 + borrowck 借用规则（可变性、悬垂、别名）接线 | ✅（含 `ref`/`ref mut` 模式与严格借用检查收尾；执行情况见 [`g1-reference.md`](tasks/leaf/g1-reference.md)、[`g2-ref-pattern.md`](tasks/leaf/g2-ref-pattern.md | [`g2-ref-pattern.md`](../tasks/leaf/g2-ref-pattern.md) |
| G2 | **`str` 切片与 String 补齐**：`&str` 引用切片（新增 `as_str()` 只读借用视图，`substring` 保持拷贝返回以免破坏现有 API）；`String::from(s)` 支持运行期 String/str 内容（长度表达），消除 §13 约束 4 | ✅（含 `str` 值一等类型补全；执行情况见 [`g2-str-slice.md`](tasks/leaf/g2-str-slice.md)、[`g2-str-value.md`](tasks/leaf/g2-str-value.md | [`g2-str-value.md`](../tasks/leaf/g2-str-value.md) |
| G3 | **裸指针**：`*const T`/`*mut T` 类型 + `*p` 读写（FFI 场景；codegen 直通） | ✅ | [`g3-raw-pointer.md`](../tasks/leaf/g3-raw-pointer.md) |
| G4 | **生命周期标注**：`'a` 参数（grammar.md 已定义）、省略规则；borrowck 生命周期检查 | ✅（MVP 语法接受；执行情况见 [`g4-lifetime.md`](tasks/leaf/g4-lifetime.md)；borrowck 生命周期检查规划中 | [`g4-lifetime.md`](../tasks/leaf/g4-lifetime.md) |

**风险与对策**：借用检查是全语言最复杂的子系统，建议分四步走——先允许（宽松借用，typecheck 报错兜底）→ 严格借用检查 → 生命周期 → 指针；每步以 `cargo test --workspace` + clippy 0 警告为准入。
