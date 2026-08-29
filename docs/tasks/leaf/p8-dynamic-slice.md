# P8 动态切片增强

> **所属专项**：[专项开发计划](../专项开发计划.md)（P8）
> **来源缺陷**：[`leaf/legacy-misc.md`](legacy-misc.md) #2（Vec<T>/数组动态切片遗留限制）
> **状态**：✅ 已完成（2026-08-29：`v[..]`/`v[..<]`/`v[..<2]`/`v[1..<]` 省略边界，Vec + String 均验证；HashMap 切片暂不实施——见下）
> **风险**：中
> **前置能力**：无

## 目标

补齐动态切片缺口：`[..]` 省略边界（`v[..]`/`v[0..]`/`v[..<3]`）+ HashMap 切片（可选）。

## 现状（2026-08-28）

- `v[lo..<hi]` / `v[lo...hi]` / `v[lo<..hi]` 在 `check_index` 特判 Range 走 `check_slice`（`check_expr/field.rs:97-392`）
- 四种切片对象：String → `String::substring`；`&str` → substring；`Vec<T>` → std `Vec::slice`；数组 `[T; N]` → 展开 Vec 拷贝循环 + clamp
- **现有限制**：
  - 仅 String/&str/Vec/数组，HashMap 不可切片（`field.rs:116-121` Unsupported）
  - **无 `[..]` 省略边界**：`ExprKind::Range` 的 `lower`/`upper` 均必填（parser `expr/mod.rs` 强制两侧表达式），`v[..]`/`v[0..]`/`v[..<3]` 不可用
  - 返回全新缓冲（值拷贝）无零拷贝视图；负索引 clamp 到 0 非倒数语义

## 技术方案

### 步骤 1：parser `ExprKind::Range` 的 `lower`/`upper` 改 `Option`

支持 `v[..]`/`v[0..]`/`v[..<3]` 省略边界。

- **涉及**：`crates/rlyeh-parser/src/expr/mod.rs`（Range 解析，245-265 行）+ `crates/rlyeh-ast/src/lib.rs`（`ExprKind::Range` 定义，322-332 行）
- **影响**：AST 结构变更，波及 `check_slice` 与所有 Range 消费点

### 步骤 2：`check_slice` 处理省略边界

缺省语义：lower 省略 = 0、upper 省略 = len（或按区间变体推导）。

- **涉及**：`check_expr/field.rs` `check_slice`
- **验收**：`v[..]`/`v[0..]`/`v[..<3]` 语义正确

### 步骤 3（可选）：HashMap 切片支持

- **风险**：低-中（需定义 HashMap 有序切片语义，MVP 可延后）
- **决策**：⏸️ **暂不实施**——HashMap 是**无序**哈希表（7 槽 Robin Hood），无稳定元素顺序，
  「切片」语义不明确（按插入序？哈希序？）。待引入有序容器（BTreeMap 已存在，
  其 `keys`/`vals` 为有序 Vec）后，切片可经 `Vec` 切片间接达成，无需为 HashMap 定义新语义。

## 波及范围

- `crates/rlyeh-parser/src/expr/mod.rs` + `crates/rlyeh-ast/src/lib.rs`（Range 改 Option）
- `check_expr/field.rs` `check_slice`
- 测试：`dynamic_slice_test.rs` / `string_slice_syntax_test.rs`

## 执行步骤

1. parser `ExprKind::Range` 的 `lower`/`upper` 改 `Option`，支持省略边界
2. `check_slice` 处理省略边界（缺省：lower=0、upper=len）
3. （可选）HashMap 切片支持

## 验收标准

- `v[..]`/`v[0..]`/`v[..<3]` 语义正确
- 回归全绿

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-28 | 由专项开发计划 P8 生成叶子文档 |
| 2026-08-29 | ✅ 完成：AST `ExprKind::Range` 的 `lower`/`upper` 改 `Option<AstExpr>`（`rlyeh-ast/src/lib.rs`）；parser 两处支持省略——`expr/mod.rs` 中缀 Range 的 upper 在 `]` 前为 None（`v[1..<]`），索引 `[` 后特判 Range 起始 token 构造 lower=None（`v[..<2]`），并在切片上下文接受旧语法 `v[..]`（全量切片，符合 Rust 直觉，其他上下文仍按废弃报错）；`typecheck/check_expr/field.rs` 的 `check_slice` 接收 `Option`，省略下界→`0`、省略上界→`i64::MAX`（依赖 std `substring`/`Vec::slice`/数组循环的 clamp：`e > len → len`）；非切片 Range 消费点（for 迭代 / `in` 判断 / 集合字面量 / 裸 Range）对 None 报「须显式边界」。适配波及：desugar（devar/rewrite/scan/expand/guard）、typecheck（mod/iter/index_enum/in_expr）、tools（rlyeh-check/rlyeh-fmt，fmt 支持省略边界格式化、`AstPattern::Range` 传 Some）、parser 测试。验证：`v[..]`/`v[..<]`全量=3、`v[..<2]`=2、`v[1..<]`=2 首元素 20、String `s[1..<]`="ello"/`s[..<2]`="he"；新增 `dynamic_slice_test` 两例（vec/string_slice_omitted_bounds）；修复 P5 泛型化遗漏的 `run-pass/mutex_guard.rl`（`Mutex::new()`→`Mutex::new(0)` + `mut`）；suite_test + cargo test 全绿。**注**：`executor_test` 的 stack overflow 为测试线程栈不足的既有环境问题（`RUST_MIN_STACK=32MB` 后 3 例全过），非本次改动引入 |
