# SH-P1-1 泛型 trait / impl 完整化

> **级别**：P1（阻塞前端自举） · **状态**：⏳ 规划中 · **归属**：0.2.0-A
> **索引**：[`../self-hosting-p1.md`](../self-hosting-p1.md) · **评估**：[`../../self-hosting/feasibility.md`](../../self-hosting/feasibility.md) §4 P1-4 · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.1

## 目标
支持**带泛型参数的 trait 声明与 impl**，并允许 impl 方法返回 `Self`，解除"注释 typecheck 自身需泛型 trait"的鸡生蛋问题，解锁前端自举。

## 技术细节
- 当前 Rlyeh 0.1.0：「trait+impl 必须非泛型」（见 `docs/guide/13-references-limits.md`）。关联类型已在 U2 落地，本任务在其上叠加泛型参数化。
- 受影响 Rust 代码（事实依据）：`rlyeh-typecheck/src/types.rs:440` `impl Trait<Args> for Type` 带泛型实参列表；`check_expr/generic.rs`（16.6K）、`closure.rs`（26K）依赖泛型实例化。
- 子任务（对应 0.2.0-A）：A1 泛型 trait 声明 / A2 泛型 impl / A3 含 `Self` 返回的 impl 方法 / A4 泛型约束 `where`/`:` 收尾。

## 受影响组件
`rlyeh-typecheck`（trait/impl 解析、泛型实例化）、前端所有需泛型 trait 的 IR 节点。

## 验证
- 单元：泛型 trait 声明 + 多 impl 单态化正确；`From::from`/`Into::into` 风格可用。
- 验收：编译器改动后 `cargo test --workspace` 全绿。

## 状态
⏳ 规划中（0.2.0 必须项，阶段 A）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P1-4 拆出为叶子 |
