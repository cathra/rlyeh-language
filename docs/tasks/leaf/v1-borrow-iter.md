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

`Iter<T> { data: *const T, len }`/`IterMut<T> { data: *mut T, cur, len }` 裸指针 + 剩余长度；`iter(&self)` 经 `let p: *const T = &self.data[0]`（V1 GEP 真实取址）取首元素地址；`next() -> Option<T>` 值拷贝读取推进、`IterMut::write(x)` 经 DerefSet 写回真实原槽；编译器特判构造 `Iter::new`/`IterMut::new`；接入 for 与 J3 适配器。替代 T1a 退化的『元素值拷贝缓冲』。

**待办（受语言限制，2026-08-27 评估）**：
- **引用元素 `Option<&T>`**：独立引用迭代器 `next() -> Option<&T>`（持 `&Vec<T>` + 游标）可行，但**泛型结构体构造需显式类型实参**（方法内无法从 `self: &Vec<T>` 推断 `IterRef<T>`），且 **Rlyeh 泛型结构体不支持静态方法（`new`）**——故 `Vec::iter_ref()` 无法在方法内构造泛型 `IterRef<T>`。需语言增强（泛型结构体构造实参推断 / 静态方法）。
- **`HashMap::iter` 引用迭代器（`(&K, &V)`）**：需元组支持（键值对），MVP 未提供——`iter()` 当前返回 `Vec<K>` 键缓冲退化，配 `values()` 使用。

## 验证

`addr_of_field_index.{rlyeh,out}` + 重写 `vec_api.rl`（iter 求和/iter_mut 写回/适配器链）+ examples 副本 + suite_test 全绿。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
