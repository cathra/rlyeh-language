# Milestone 2 — 生产可用（Month 4-8）

> **里程碑范围**：M2.1–M2.8 + 已完成语言特性
> **所属任务树**：[任务文档导航](./README.md) → 里程碑列表（[`CODEBUDDY.md`](../../CODEBUDDY.md) §6）
> **本层职责**：该里程碑细化后的**任务列表**（每个任务一个单任务文档，见 milestone-tasks/）。

> 「生产可用」里程碑：Actor 运行时、包管理器、增量编译、标准库核心、交叉编译/WASM 多目标、发布流程与智能区域。

---

## 任务列表

| 任务 | 内容 | 状态 | 单任务文档 |
|------|------|------|-----------|
| M2.1 | Actor 运行时 | ✅ | [`m2-1.md`](milestone-tasks/m2-1.md) |
| M2.2 | 包管理器 Dagon | ✅ | [`m2-2.md`](milestone-tasks/m2-2.md) |
| M2.3 | 增量编译引擎 | ✅ | [`m2-3.md`](milestone-tasks/m2-3.md) |
| M2.4 | LSP 服务器 | 📋 | [`m2-4.md`](milestone-tasks/m2-4.md) |
| M2.5 | 标准库核心模块 | ✅ | [`m2-5.md`](milestone-tasks/m2-5.md) |
| M2.6 | 交叉编译 | ✅ | [`m2-6.md`](milestone-tasks/m2-6.md) |
| M2.7 | WASM 目标 | ✅ | [`m2-7.md`](milestone-tasks/m2-7.md) |
| M2.8 | 智能区域 | ✅ | [`m2-8.md`](milestone-tasks/m2-8.md) |

## 未编号特性/任务（无编号，已完成）

| 特性 | 状态 | 相关文档 |
|------|------|---------|
| 模块系统 | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| E3 完成 发布流程 | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| 聚合对象语言特性 | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| for 循环 range 迭代器 | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| `for ... in` Vec 容器迭代 + `v[i]` 索引访问 | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| `for (k, v) in m` HashMap 元组模式迭代 | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| `String` 内容相等比较 `==`/`!=` | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| 修复：LIR 嵌套 Binary 比较结果错登记为 i64 槽 | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| `HashMap` 字符串键（djb2 内容哈希） | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| `String` 拼接 `+` 运算符 + `push_str` | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| `String` 字典序比较 `<`/`>`/`<=`/`>=` | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| `String` 子串/查找：`substring` + `find` + `contains` | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| `String` 范围切片语法 `s[lo..<hi]`/`s[lo...hi]`/`s[lo<..hi]` | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| `String` 大小写/裁剪：`to_upper` + `to_lower` + `trim` | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| `String` 常用操作：`starts_with` + `ends_with` + `replace` | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| `String` 数值转换：`int_to_string` + `string_to_int` | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| `Vec<T>` 常用操作：`contains` + `remove` + `insert` + `clear` | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| `Vec<T>` 查找/排序：`find` + `sort` | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| `String` 分割/重复/填充：`split` + `repeat` + `pad_start` + `pad_end` | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| 修复：LIR 跨函数返回类型推断——「返回用户函数调用结果」的函数被误判为 i64 | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| 修复：MIR `lower_if` 条件为含控制流的块表达式时「基本块已有终止符」 | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| 跨模块路径表达式 | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| 数组字面量与索引访问 | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| 标准库 `Vec<T>` 动态数组 | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| 标准库 `String` 动态字符串 | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |
| 标准库 `HashMap<K,V>` 开放寻址哈希表 | ✅ | 见 [`leaf/`](leaf/) 阶段任务 |

---

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 里程碑任务列表细化为单任务文档（milestone-tasks/），本文件改为任务列表 |
