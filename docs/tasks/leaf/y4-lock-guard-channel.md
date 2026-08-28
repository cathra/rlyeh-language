# Y4 锁 guard 完整 + Channel 泛型化

> **所属阶段**：阶段 Y
> **状态**：📋 规划（已拆分 Y4a/Y4b/Y4c；探测发现泛型 struct 字面量构造语言级障碍，2026-08-28）
> **依赖**：U3
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`Mutex<T>`/`RwLock<T>` 泛型化 + guard + `Channel<T>` 泛型化 + 有界队列。

## 背景

阶段 阶段 Y 子任务，详见 阶段详情文档 [`stages/Y.md`](../../stages/Y.md)。

## 技术细节

`Mutex<T>`/`RwLock<T>` 泛型化（目标签名 `lock(&self) -> MutexGuard<T>`，替代裸 lock/unlock + lock_guard() 命名特判）；`RwLockWriteGuard`/`RwLockReadGuard`；`Deref`/`DerefMut` 语义（`*guard` 解引用访问数据）；`Channel<T>` 泛型化、`bounded_channel(capacity)` 有界队列、`SendError<T>`/`RecvError`/`TryRecvError` 错误类型、`Arc<LockFreeQueue>`。

## 风险探测（2026-08-28）

**语言级障碍：泛型 struct 字面量构造失败。**
- 实测：`struct MutexGuard<T> { value: T }` + `let g: MutexGuard<i64> = MutexGuard { value: 42 };` → typecheck 报 `argument 0 of struct MutexGuard field value expects T, found i64`。
- **即使显式类型注解 `MutexGuard<i64>`，字面量构造仍无法实例化泛型参数 `T`**（`T` 未被替换为具体类型）。
- std 泛型类型（`Vec<T>`/`Result<T,E>`/`HashMap<K,V>`）经内建/特判构造，用户自定义泛型 struct 的字面量构造缺泛型实例化支持。
- **影响**：Y4 的 `MutexGuard<T>`/`Mutex<T>` 泛型锁无法按目标签名实现，除非先修语言级泛型构造。

## 拆分方案（高 → 中低风险粒度）

### Y4a（低风险，先破解语言级障碍）
- **语言级**：泛型 struct 字面量构造的泛型参数实例化（`MutexGuard<i64> { value: 42 }` 的 `T` 实例化）。此为 Y4 全体的前提，单独推进可独立验证。

### Y4b（中风险，依赖 Y4a）
- `Mutex<T>`/`RwLock<T>` 泛型声明 + 泛型 `lock(&self) -> MutexGuard<T>` 签名（替代裸 lock/unlock + `lock_guard()` 命名特判）；`RwLockWriteGuard`/`RwLockReadGuard`；`Deref`/`DerefMut` 语义（`*guard` 解引用——Deref trait 需确认可定义 + 解引用运算符支持）。

### Y4c（中风险，独立于 Y4a/Y4b）
- `Channel<T>` 泛型化（元素不再限 i64）+ `SendError<T>`/`RecvError`/`TryRecvError` 错误类型 + `bounded_channel(capacity)` 有界队列 + `Arc<LockFreeQueue>`。Channel 泛型化可独立于 Mutex guard 推进。

## 验证

`mutex_generic.{rlyeh,out}`（泛型 Mutex + Deref guard + bounded Channel + 错误类型）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
| 2026-08-28 | 风险探测（泛型 struct 字面量构造障碍）+ 拆分 Y4a/Y4b/Y4c |
