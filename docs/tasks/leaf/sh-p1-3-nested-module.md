# SH-P1-3 嵌套模块 / `pub use` / `super`

> **级别**：P1（阻塞前端自举） · **状态**：⏳ 规划中 · **归属**：0.2.0-B
> **索引**：[`../self-hosting-p1.md`](../self-hosting-p1.md) · **评估**：[`../../self-hosting/feasibility.md`](../../self-hosting/feasibility.md) §4 P1-6 · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.2

## 目标
支持**嵌套模块**、`pub use` 重导出、`super`/`crate::` 相对路径与模块级可见性，使 Rlyeh 侧能用层级化命名空间组织大型编译器 crate（替代当前扁平命名空间）。

## 技术细节
- 当前 Rlyeh 0.1.0：仅顶层 `module` + 扁平名字空间 + 别名导入/重命名；**不支持 `pub use`/相对 `super`/嵌套模块**（见 `docs/guide/13-references-limits.md` §模块）。
- 受影响 Rust 代码（事实依据，来自 `crates/` 核查）：
  - `rlyeh-driver/src/lib.rs:17-21` `pub mod error; pub mod incremental; mod module;`
  - `rlyeh-typecheck/src/check_expr/{mod,call,method,...}.rs`（32 文件多级模块）
  - `rlyeh-region-alloc/src/lib.rs:46-59` `mod block; mod bump; pub mod allocator; ...`
- 子任务（对应 0.2.0-B）：B1 嵌套模块 / B2 `pub use` / B3 `super`/`crate::` / B4 可见性细化。

## 受影响组件
所有多文件 crate（driver / typecheck / region-alloc / 后续 Rlyeh 侧重写组件）。

## 验证
- 单元：多级模块可访问；`pub use` 重导出符号可见；相对路径解析正确；越权访问报错。
- 验收：编译器改动后 `cargo test --workspace` 全绿。

## 状态
⏳ 规划中（0.2.0 必须项，阶段 B）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P1-6 拆出为叶子 |
