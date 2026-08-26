# 阶段 G–L（编译器能力补齐）

> **所属任务树**：[任务文档导航](./README.md)
> **状态**：✅ 全部完成
> **权威来源**：阶段详情文档 [`stages/G.md`](../stages/G.md)、[`stages/L.md`](../stages/L.md)；执行记录见本文件 §执行记录
> **本层职责**：记录阶段 G–L 的任务列表与进度 + 执行记录；具体实施见权威源文档（development-plan.md）。

---

## 任务列表与进度

| 阶段 | 主题 | 关键交付 | 依赖 | 状态 |
|------|------|---------|------|------|
| **G** | 引用与借用（L0 完整化） | `&T`/`&mut T`、`*` 解引用、`str` 切片、裸指针、生命周期 `'a` | 无（地基） | G1–G4 ✅（G4 为语法接受 MVP，borrowck 生命周期检查规划中） |
| **H** | 一等函数 | `fn(A) -> B` 函数类型、闭包（捕获 + `move`）、`dyn Trait` | G（引用捕获） | H1–H5 ✅（H3 IIFE MVP；H4 dyn Trait MVP；H5 闭包值对象 MVP） |
| **I** | 宏系统与格式化 | `macro_rules!` 声明式宏、`Display`/`Debug`、`println!`/`format!` | 弱（可与 G/H 并行） | ✅ 已完成 |
| **J** | 迭代器与集合协议 | 数组迭代、`Iterator` trait、`map`/`filter`/`fold`/`collect` | H（适配器闭包） | J1–J3 ✅ |
| **K** | 错误传播与所有权层级 | `?` 运算符、`Box<T>`、`Rc<T>`/`Arc<T>`、`Gc<T>` | G（指针操作） | K1–K4 ✅ |
| **L** | 生态模块与平台收尾 | `async fn`/`await`、`serde`、region `strategy (bump)`、WASI net / Actor 交叉编译 | I（serde 宏）、G 等 | L1–L4 ✅ |

---

## 阶段详情索引

- 阶段 G–L 详细任务：阶段详情文档 [`stages/G.md`](../stages/G.md)、[`stages/L.md`](../stages/L.md)
- 阶段 G–L 执行记录：见本文件 §执行记录

---

## 子任务列表（按阶段分层）

> 每个阶段下列出其子任务（一个任务一个叶子文档，含具体执行情况与技术细节）：

### G — 引用与借用

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| G1 引用类型与表达式 | [`g1-reference.md`](leaf/g1-reference.md) | ✅ 已完成 |
| G2 `str` 切片与 String 补齐 | [`g2-str-slice.md`](leaf/g2-str-slice.md) | ✅ 已完成 |
| G2 补全 `str` 值一等类型 + 实参自动升级 | [`g2-str-value.md`](leaf/g2-str-value.md) | ✅ 已完成 |
| G1 收尾 `ref`/`ref mut` 模式 | [`g2-ref-pattern.md`](leaf/g2-ref-pattern.md) | ✅ 已完成 |
| G3 裸指针 | [`g3-raw-pointer.md`](leaf/g3-raw-pointer.md) | ✅ 已完成 |
| G4 生命周期标注 | [`g4-lifetime.md`](leaf/g4-lifetime.md) | ✅ 已完成 |

### H — 一等函数

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| H1 函数类型与函数指针 | [`h1-fn-pointer.md`](leaf/h1-fn-pointer.md) | ✅ 已完成 |
| H2 无捕获闭包 | [`h2-closure.md`](leaf/h2-closure.md) | ✅ 已完成 |
| H3 捕获闭包（IIFE MVP） | [`h3-capture-closure.md`](leaf/h3-capture-closure.md) | ✅ 已完成 |
| H4 `dyn Trait` trait 对象 | [`h4-dyn-trait.md`](leaf/h4-dyn-trait.md) | ✅ 已完成 |
| H5 闭包值对象 | [`h5-closure-value.md`](leaf/h5-closure-value.md) | ✅ 已完成 |

### I — 宏系统与格式化

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| I1 声明式宏 | [`i1-declarative-macro.md`](leaf/i1-declarative-macro.md) | ✅ 已完成 |
| I2 内置格式化宏 | [`i2-format-macro.md`](leaf/i2-format-macro.md) | ✅ 已完成 |
| I3 集合宏 `arr!`/`vec!`/`map!` | [`i3-collection-macro.md`](leaf/i3-collection-macro.md) | ✅ 已完成 |

### J — 迭代器与集合协议

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| J1 数组迭代 | [`j1-array-iter.md`](leaf/j1-array-iter.md) | ✅ 已完成 |
| J2 自定义迭代器接入 for | [`j2-custom-iter.md`](leaf/j2-custom-iter.md) | ✅ 已完成 |
| J3 适配器 | [`j3-adapters.md`](leaf/j3-adapters.md) | ✅ 已完成 |

### K — 错误传播与所有权

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| K1 `?` 错误传播运算符 | [`k1-question.md`](leaf/k1-question.md) | ✅ 已完成 |
| K2 `Box<T>` 堆分配装箱 | [`k2-box.md`](leaf/k2-box.md) | ✅ 已完成 |
| K3 `Rc<T>`/`Arc<T>` 引用计数装箱 | [`k3-rc-arc.md`](leaf/k3-rc-arc.md) | ✅ 已完成 |
| K4 `Gc<T>` 追踪 GC（MVP 保守标记-清除） | [`k4-gc.md`](leaf/k4-gc.md) | ✅ 已完成 |

### L — 生态模块与平台收尾

| 子任务 | 叶子文档 | 状态 |
|--------|---------|------|
| L1 普通函数 `async fn`/`await` | [`l1-async-fn.md`](leaf/l1-async-fn.md) | ✅ 已完成 |
| L2 `serde` 序列化模块 | [`l2-serde.md`](leaf/l2-serde.md) | ✅ 已完成 |
| L3 region 选项接线 | [`l3-region.md`](leaf/l3-region.md) | ✅ 已完成 |
| L4 平台加固（WASI net + Actor 交叉编译/WASM） | [`l4-platform.md`](leaf/l4-platform.md) | ✅ 已完成 |

---
## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 阶段 G–L 纳入任务树（索引，权威源 development-plan.md） |
| 2026-08-26 | 执行记录归档：搬入 development-plan §4 + CODEBUDDY §5.5（G–L）到本小节，源文档对应区替换为链接 |
