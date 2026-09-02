# SH-P2-3 `Box` 深树 + 内部可变性（Rc/Arc/RefCell 等价 / arena）

> **级别**：P2（可行但需重写 / 外部依赖） · **状态**：🟢 核心落地 · **归属**：0.2.0-I
> **索引**：[`../self-hosting-p2.md`](../self-hosting-p2.md) · **评估**：[`../../self-hosting/feasibility.md`](../../self-hosting/feasibility.md) §4 P2-9

## 目标
为 Rlyeh 侧重写编译器时提供**树形 IR 的可变遍历**方案——补齐 `RefCell` 等价或采用 arena/索引式表示，替代 Rust 的 `Rc`/`Arc`/`RefCell` 内部可变性模式。

## 技术细节
- 当前 Rlyeh 0.1.0 有 `Box<T>`/`Rc<T>`/`Arc<T>`/`Weak<T>`/`Gc<T>`（K2–K4），但**无 `RefCell`**；文档已知倾向**索引式**表示（数组/Vec 适配器因数组非命名类型保留内建 desugar）。
- 受影响 Rust 代码（事实依据，来自 `crates/` 核查）：
  - `rlyeh-ast`/`rlyeh-parser`：`Box` 深 AST 树。
  - `rlyeh-actor-runtime`：`Arc<Mutex<...>>`/`Weak`（并发状态）。
  - `rlyeh-driver` 增量编译：缓存内部可变性。
- 可选方案：(a) 引入 `RefCell` 等内部可变性原语；(b) 改用 arena + 整数索引（`Arena<T>` + `NodeId`），与 Rlyeh 索引式倾向一致，避免运行时引用计数开销。

## 受影响组件
所有 IR crate（ast/hir/mir/lir/desugar/typecheck）、driver 增量编译。

## 验证
- 单元：Rlyeh 侧用 arena/索引或内部可变性原语完成一次 AST 可变重写遍历，结果等价于 Rust 版。
- 集成：typecheck 在 arena 表示下产出与现版一致的类型。
- **（🟢 已完成，2026-09-02）** I2：泛型 `Arena<T>` + `NodeId`（`Vec<T>` 槽区 + 1-based 下标），整型二叉树 IR 经显式工作栈完成可变重写遍历（`tests/run-pass/arena-ir-traversal.rl`）；I1：`RefCell<T>` 安全内部可变性封装（`unsafe` 裸指针 + `alloc_array` 堆缓冲，`get(&self)`/`set(&self)` 经共享引用 mutate，即得 `Rc<RefCell<T>>` 共享可变状态，`tests/run-pass/refcell.rl`）。全量 `rlyeh test tests` **255/255 通过**。
- **已知限制**：泛型 impl 关联静态方法 MVP 不支持（`Vec::new()` 特判在泛型关联函数内无法解析元素类型），故空 `Arena` 在调用方顶层以 `Arena { slots: Vec::new() }` 构造（见 `arena-ir-traversal.rl` 注释）。

## 状态
🟢 核心落地（I2 arena + `NodeId` 与 I1 `RefCell` 内部可变性均已验证；见变更记录）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P2-9 拆出为叶子 |
| 2026-09-02 | 实现 I2 arena + `NodeId`（泛型 `Arena<T>`/`NodeId`/可变遍历）+ I1 `RefCell<T>` 内部可变性；新增 run-pass `arena-ir-traversal.rl`/`refcell.rl`；状态由规划中改为核心落地 |
