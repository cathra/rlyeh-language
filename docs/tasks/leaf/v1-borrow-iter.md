# V1 借用迭代器瘦指针 MVP

> **所属阶段**：阶段 V
> **状态**：✅ 已完成（`Vec::iter`/`iter_mut`/`iter_ref` + `HashMap::iter_pairs` 零拷贝引用迭代均落地；元组运行时未就绪，键值对以 `KVRef` 结构体承载，等价 `(&K, &V)` 引用语义）
> **依赖**：U1/U2/U3
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`Vec::iter() -> Iter<T>`/`iter_mut() -> IterMut<T>` 零分配零拷贝视图。

## 背景

阶段 阶段 V 子任务，详见 阶段详情文档 [`stages/V.md`](../../stages/V.md)。

## 技术细节

`Iter<T> { data: *const T, len }`/`IterMut<T> { data: *mut T, cur, len }` 裸指针 + 剩余长度；`iter(&self)` 经 `let p: *const T = &self.data[0]`（V1 GEP 真实取址）取首元素地址；`next() -> Option<T>` 值拷贝读取推进、`IterMut::write(x)` 经 DerefSet 写回真实原槽；编译器特判构造 `Iter::new`/`IterMut::new`；接入 for 与 J3 适配器。替代 T1a 退化的『元素值拷贝缓冲』。

**进度（2026-08-29 更新）**：
- **引用元素 `Option<&T>` ✅ 已完成**：`Vec::iter_ref() -> IterRef<T>`，`IterRef::next() -> Option<&T>` 零拷贝引用（指向原缓冲真实槽），支持只读读取与 `*r = x` 原地写回；for 循环经 inherent next 接入。实现路径：将 `Iter`/`IterMut` 已有的编译器构造器特判（`call.rs` / `construct.rs`）扩展至 `IterRef`，并新增裸指针索引 `p[i]`（`check_index` 的 `Type::RawPtr` 分支）以支撑 `&p[0]` 取元素引用。新增回归用例 `vec_iter_ref.rl`（全量 176 通过，0 回归）。原评估「泛型结构体不支持静态方法」已过时——`Iter::new` 本就是编译器特判构造，扩展同名特判即可。
- **`HashMap` 键值对引用迭代 ✅ 已完成（2026-08-29）**：Rlyeh **元组在 codegen 层完全无支持**（crates/rlyeh-codegen 中零 `Tuple` 分支），故 `(&K, &V)` 字面返回不可行。改用 `HashMap::iter_pairs() -> HashMapIter<K, V>`，`next() -> Option<KVRef<K, V>>`（`KVRef` 含 `key: *const K`、`val: *const V` 两裸指针，指向原 `keys`/`vals` 真实槽，零拷贝），等价 Rust `(&K, &V)` 引用语义。迭代器持有 `states/keys/vals` 三数组指针 + 游标，`next()` 跳过 `states != 1` 的空/墓碑槽（已用 `remove` 后墓碑跳过验证）。既有 `iter()`（返回 `Vec<K>` 键缓冲）/ `keys()`/`values()` 签名保持不变（测试依赖）。实现依赖 `check_struct_construct` 的泛型结构体字面量构造——为此给 `unify` 增加 `RawPtr` 递归统一（与既有 `Ref` 分支对称），使 `*const T` 字段可反推泛型参数。新增回归用例 `hashmap_iter_pairs.rl`（全量 177 通过，0 回归）。

## 验证

`addr_of_field_index.{rlyeh,out}` + 重写 `vec_api.rl`（iter 求和/iter_mut 写回/适配器链）+ examples 副本 + suite_test 全绿。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
| 2026-08-29 | V1 全部完成：`Vec::iter_ref`（`Option<&T>`）已实现；`HashMap::iter_pairs`（`KVRef` 零拷贝 KV 引用迭代，等价 `(&K,&V)`）已实现；`unify` 增加 `RawPtr` 递归统一，支撑泛型结构体裸指针字段反推类型参数 |
