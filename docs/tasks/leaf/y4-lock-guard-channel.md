# Y4 锁 guard 完整 + Channel 泛型化

> **所属阶段**：阶段 Y
> **状态**：🔧 部分完成（Y4a 泛型构造推断 + Y4b-1 Deref 解引用分派 ✅，2026-08-28，163 用例全绿；Y4b-2/3/4 锁 guard / Y4c Channel 待办）
> **依赖**：U3
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`Mutex<T>`/`RwLock<T>` 泛型化 + guard + `Channel<T>` 泛型化 + 有界队列。

## 背景

阶段 阶段 Y 子任务，详见 阶段详情文档 [`stages/Y.md`](../../stages/Y.md)。

## 技术细节

`Mutex<T>`/`RwLock<T>` 泛型化（目标签名 `lock(&self) -> MutexGuard<T>`，替代裸 lock/unlock + lock_guard() 命名特判）；`RwLockWriteGuard`/`RwLockReadGuard`；`Deref`/`DerefMut` 语义（`*guard` 解引用访问数据）；`Channel<T>` 泛型化、`bounded_channel(capacity)` 有界队列、`SendError<T>`/`RecvError`/`TryRecvError` 错误类型、`Arc<LockFreeQueue>`。

## 风险探测与破解（2026-08-28）

**语言级障碍：泛型 struct 字面量构造失败（已破解，Y4a）。**
- 探测：`struct MutexGuard<T> { value: T }` + `let g: MutexGuard<i64> = MutexGuard { value: 42 };` → typecheck 报 `argument 0 of struct MutexGuard field value expects T, found i64`。即使显式类型注解 `MutexGuard<i64>`，`T` 仍未被实例化。
- **破解（Y4a）**：construct.rs `check_struct_construct` 在 `type_args` 为空且 struct 含泛型参数时，从字段实参推断（字段类型裸 `Generic(tp)` 的直接推断）。`MutexGuard { value: 42 }` → value: T → T = i64。
- **遗留限制**：复合字段（`Vec<T>`/`HashMap<K,V>`）的统一推断暂不覆盖（登记 lang-defects.md）。

## 拆分方案（高 → 中低风险粒度）

### Y4a（✅ 已完成，2026-08-28）
- **语言级**：泛型 struct 字面量构造的泛型参数实例化（construct.rs 字段实参推断）。测试 `y4a_generic_struct.{rl,out}`：`MutexGuard { value: 42 }` → 42、`MutexGuard { value: String::from("hi") }` → hi、显式 turbofish 仍可用。

### Y4b（中风险，依赖 Y4a；细化拆分 Y4b-1/2/3/4，2026-08-28）
- **探测**：`Deref<T>` trait 定义 + `guard.deref()` **直接调用可行**（输出 42）；但 `*guard` 解引用运算符仅支持内建类型（`&T`/裸指针/`Box`/`Rc`/`Arc`/`Gc`），不识别自定义 Deref trait（typecheck `UnaryOp::Deref` 经 `heap_wrapper_inner` 特判）。
- **Y4b-1（✅ 已完成，语言级，2026-08-28）**：`UnaryOp::Deref`（mod.rs）非内建堆装箱时查 o_ty 的 `deref` 方法（`find_impl_for_method`），生成 `o.deref()` 调用（经 `check_method_call`），codegen 走正常方法路径无需改。测试 `y4b_deref.{rl,out}`：`*guard` 解引用 i64/String + 显式 `deref()` 等义，`42`/`hi`/`7`。
- **Y4b-2（✅ 语言级 + 原型完成，2026-08-28；std 迁移待专项）**：method.rs 泛型 impl 静态方法从实参推断类型参数（`MyMutex::new(42)` → `v: T` → T = i64，替代 MVP unsupported）。原型验证 `y4b_mutex_generic.{rl,out}`：`MyMutex::new` + `lock` + `*guard` 解引用（i64/String，`42`/`hi`）。**std `Mutex` 泛型化为破坏性改动**（波及 core.rl/guard.desugar/driver sync_test），登记 lang-defects 待专项。
- **Y4b-3（std，中）**：`RwLock<T>` 泛型化 + `RwLockReadGuard`/`RwLockWriteGuard` 实现 Deref。
- **Y4b-4（std，低）**：guard 可变访问（`DerefMut` 语义或 `get_mut` 访问器）。

### Y4c（📋 暂缓，登记 lang-defects #8，2026-08-28）
- `Channel<T>` 泛型化（元素不再限 i64）+ `SendError<T>`/`RecvError`/`TryRecvError` 错误类型 + `bounded_channel(capacity)` 有界队列 + `Arc<LockFreeQueue>`。
- **探测**：`Channel<T> { queue: Vec<T> }` 构造触发复合字段推断障碍（`Vec<T>` 无法从空 `Vec::new()` 推断 T；Y4a 仅裸泛型字段）且无 turbofish（`Channel::<i64>::new()` 语法错误）。登记 [`lang-defects.md`](./lang-defects.md) #8 待专项（复合字段推断 + 泛型返回类型推断）。

## 验证

`y4a_generic_struct.{rl,out}`（泛型推断构造）+ `y4b_deref.{rl,out}`（Deref 解引用分派）+ `y4b_mutex_generic.{rl,out}`（泛型 Mutex 原型，164 用例全绿）；`mutex_generic.{rlyeh,out}`（泛型 Mutex + Deref guard + bounded Channel + 错误类型，Y4b-3/4 + Y4c 后续）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
| 2026-08-28 | 风险探测（泛型 struct 字面量构造障碍）+ 拆分 Y4a/Y4b/Y4c |
| 2026-08-28 | Y4a 泛型 struct 字面量构造推断完成（162 用例全绿） |
| 2026-08-28 | Y4b-1 Deref 解引用分派完成 + 细化 Y4b-2/3/4（163 用例全绿） |
| 2026-08-28 | Y4b-2 泛型 impl 静态方法推断完成 + 泛型 Mutex 原型（164 用例全绿；std 迁移待专项） |
| 2026-08-28 | Y4c 探测（复合字段推断 + 无 turbofish 障碍）登记 lang-defects #8，暂缓 |
