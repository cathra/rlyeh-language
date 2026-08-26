# T1a Vec 目标 API 补齐

> **所属阶段**：阶段 T
> **状态**：✅ 已完成
> **依赖**：J、V1
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

Vec 目标 API：`sort`/`binary_search`/`iter`/`get_mut`/`sort_by`。

## 背景

阶段 阶段 T 子任务，详见 阶段详情文档 [`stages/T.md`](../../stages/T.md)。

## 技术细节

`sort`/`binary_search` 已有；新增 `iter`/`iter_mut`（V1 瘦指针迭代器 `Iter<T> { data: *const T, len }`/`IterMut<T> { data: *mut T, cur, len }`，零分配零拷贝，`next() -> Option<T>` 值拷贝 + `IterMut::write(x)` DerefSet 写回；编译器特判 `Iter::new`/`IterMut::new`；接入 for 与 J3 适配器）、`get_mut`（值拷贝，目标 `&mut T` 引用规划）、`sort_by`（比较器闭包 `fn(T, T) -> i64` 三态语义，选择排序）。附带修复：typecheck `substitute`/`unify` 补 `Type::Fn` 递归分支 + 实例方法实参循环补闭包特判。

## 验证

`vec_api.{rlyeh,out}`（iter 快照 + for 求和、get_mut 值拷贝、sort_by 闭包降序/升序）+ 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
