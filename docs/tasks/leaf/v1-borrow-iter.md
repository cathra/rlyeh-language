# V1 借用迭代器瘦指针 MVP

> **所属阶段**：阶段 V
> **状态**：✅ 已完成
> **依赖**：U1/U2/U3
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`Vec::iter() -> Iter<T>`/`iter_mut() -> IterMut<T>` 零分配零拷贝视图。

## 背景

阶段 阶段 V 子任务，详见 阶段详情文档 [`stages/V.md`](../../stages/V.md)。

## 技术细节

`Iter<T> { data: *const T, len }`/`IterMut<T> { data: *mut T, cur, len }` 裸指针 + 剩余长度；`iter(&self)` 经 `let p: *const T = &self.data[0]`（V1 GEP 真实取址）取首元素地址；`next() -> Option<T>` 值拷贝读取推进、`IterMut::write(x)` 经 DerefSet 写回真实原槽；编译器特判构造 `Iter::new`/`IterMut::new`；接入 for 与 J3 适配器。替代 T1a 退化的『元素值拷贝缓冲』。待办：引用元素 `Option<&T>`、`HashMap::iter`。

## 验证

`addr_of_field_index.{rlyeh,out}` + 重写 `vec_api.rl`（iter 求和/iter_mut 写回/适配器链）+ examples 副本 + suite_test 全绿。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
