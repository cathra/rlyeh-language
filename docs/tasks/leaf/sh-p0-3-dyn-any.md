# SH-P0-3 `dyn Trait` 含 `Self` 方法 + `Any` 类型擦除

> **级别**：P0（阻塞全栈自举） · **风险**：🔴 高 · **状态**：⏳ 规划中 · **归属**：0.2.0-G
> **索引**：[`../self-hosting-p0.md`](../self-hosting-p0.md) · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.7

## 目标
支持 `dyn Trait` **调用含 `Self` 签名的方法**，并提供 `std::any::Any` 式的**类型擦除 / `downcast`**，使 actor 消息协议（异构消息信封）可用 Rlyeh 表达。

## 技术细节
- 当前 Rlyeh 0.1.0：「`dyn Trait` 约束：trait 与 impl 均须非泛型；方法签名含 `Self`（关联返回类型 / 参数）不支持经 dyn 调用」（见 `CODEBUDDY.md` §3.8 H4）。且**无类型擦除**机制。
- 受影响 Rust 代码（事实依据）：
  - `rlyeh-actor-runtime/src/actor.rs:79` `pub trait ActorState: Any + Send + Sync + 'static`
  - `rlyeh-actor-runtime/src/runtime.rs:31` `Arc<Mutex<Option<Box<dyn ActorState>>>>`、`:87` `Box<dyn Any + Send>`
  - `rlyeh-actor-runtime/src/envelope.rs:21,23` `Box<dyn Any + Send>` + `Sender<Box<dyn Any + Send>>`
  - 编译器内部已把 `dyn Trait` 建模为「数据指针 + vtable 胖指针 2 槽」（`rlyeh-typecheck/src/types.rs:66`）。
- 需设计：vtable 中 `Self` 方法的签名擦除/恢复、`Any` 类型标识存储与 `downcast` 安全检查。

## 受影响组件
`rlyeh-actor-runtime`（actor 状态 trait / 消息信封 / 类型擦除分发）。

## 验证
- 单元：Rlyeh 侧经 `dyn Trait` 调用含 `Self` 返回的方法；`Any` 装箱 + `downcast` 往返成功。
- 对拍：等价于 actor 消息 `handle_message(&mut self, ...)` 分发。

## 状态
⏳ 规划中（**0.2.0 必须项（语言特性）**，对应阶段 G；运行时*重写*属 0.3.0）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P0-3 拆出为叶子 |
| 2026-09-01 | 修正归属：由「0.3.0+ 长期跟踪」上移为 0.2.0-G（与计划 §1/§3.7 一致）；索引由 feasibility 评估改为 development-plan §3.7 |
