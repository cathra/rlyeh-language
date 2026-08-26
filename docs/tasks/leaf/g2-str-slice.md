# G2 `str` 切片与 String 补齐

> **所属阶段**：阶段 G
> **状态**：✅ 已完成
> **依赖**：G1
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

`&str` 引用切片 + `String::from` 运行期内容 + `str` 值一等类型。

## 背景

阶段 阶段 G 子任务，详见 阶段详情文档 [`stages/G.md`](../../stages/G.md)。

## 技术细节

`String::from(runtime)` desugar 为 `s.clone()` 深拷贝；`&str` 视图（运行时 = 指向 String 对象的瘦指针，复用 G1 聚合引用）：`as_str()` 特判 Ref、`resolve_ast_type` 识别 `str`、方法/索引/切片归一 String、`String::from(&str)` 读 data/len 深拷贝、`compatible_with` 允许 `&str`↔`&String`。**设计决策**：`substring` 保持拷贝返回，零拷贝以 `as_str()` 体现。

## 验证

`string_from_runtime_test.rs` 6 + `str_ref_test.rs` 9 + 全量 114 套件。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
