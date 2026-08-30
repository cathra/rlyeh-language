# Y4 锁 guard 完整 + Channel 泛型化

> **所属阶段**：阶段 Y
> **状态**：🔧 部分完成（Y4a 泛型构造推断 + Y4b-1 Deref 解引用分派 ✅，2026-08-28；Y4b-2 std Mutex 泛型化（lang-defects #6，P5，2026-08-29）✅ + Y4c Channel<T> 泛型化（lang-defects #8，P7c，2026-08-29）✅，173 用例全绿；**Y4b-3 `RwLock<T>` 泛型化 + 读/写守卫 Deref ✅（2026-08-30）**；**Y4b-4 guard 可变访问 DerefMut ✅（2026-08-30）：`*guard = x` 经 `deref_mut` 分发**；**有界队列 bounded_channel ✅（2026-08-30）**；**错误类型 SendError/RecvError/TryRecvError ✅（2026-08-30）**；**Arc<LockFreeQueue> ✅（2026-08-30，类型层：Sender/Receiver 持 `Arc<Channel<T>>` 原子引用计数，可跨线程安全共享；跨线程*移动* Sender/Receiver 受 `Thread::start(fn() -> i64)` 无参 API 限制，待线程传参能力补齐）**）
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
- **Y4b-2（✅ 语言级 + 原型完成，2026-08-28；std 迁移已完成，lang-defects #6 / P5，2026-08-29）**：method.rs 泛型 impl 静态方法从实参推断类型参数（`MyMutex::new(42)` → `v: T` → T = i64，替代 MVP unsupported）。原型验证 `y4b_mutex_generic.{rl,out}`：`MyMutex::new` + `lock` + `*guard` 解引用（i64/String，`42`/`hi`）。**std `Mutex` 泛型化已完成**：`sync/module.rl` 的 `Mutex<T> { p, value: T }` 携值 + `new(v)`/`lock_guard`/`get`/`get_mut` 落地，验收 `tests/run-pass/mutex_value.rl`（输出 `42/100/0`），173 用例全绿；原破坏性迁移（core.rl/guard.desugar/driver sync_test）已落地，详见 lang-defects #6。
- **Y4b-3（✅ 已完成，std，2026-08-30）**：`RwLock<T>` 泛型化 + `RwLockReadGuard`/`RwLockWriteGuard` 实现 Deref
  - `sync/module.rl`：`RwLock<T> { p, value: T }` 带值锁（复用 `Mutex<T>` 模式），`new(v: T)` 初始化；`read_guard(&self)` / `write_guard(&mut self)` 返回 `RwLockReadGuard<T>` / `RwLockWriteGuard<T>`（字段 `value: &T` / `&mut T`，借自 `RwLock.value`，与 `MutexGuard` 同构）；守卫 `unlock(self)` + `get` / `get_mut`，`deref(&self) -> T`（`*g` 经 typecheck `UnaryOp::Deref` 分派生成 `g.deref()` 返回 T 拷贝，约定同 `y4b_deref`）。
  - guard 自动解锁 desugar（`rlyeh-desugar/src/guard.rs` `is_lock_guard_call`）扩展识别 `read_guard` / `write_guard`，块尾注入 `g.unlock()`（原仅特判 `lock_guard`）。
  - 验证 `tests/run-pass/rwlock_value.rl`：读守卫 `*g` Deref 读 42、写守卫 `get_mut` 改 77 后 `*wg` 读 77、再读守卫读回 77 + `get()`，作用域自动解锁无死锁（`sync_test.rs` 既有 `RwLock::new()` 三例改为 `RwLock::new(0)` 适配泛型，7/7 通过）。
- **Y4b-4（✅ 已完成，std，2026-08-30）**：guard 可变访问——`get_mut` 访问器（Y4b-3 已落地）+ `*guard = x` DerefMut 分发
  - `sync/module.rl`：`MutexGuard`/`RwLockWriteGuard` 增 `deref_mut(&mut self) -> &mut T`（返回 `value` 引用）；`MutexGuard` 同时补 `deref(&self) -> T`，使 `*g` 读对所有守卫一致生效（std MutexGuard 此前仅有 `get`/`get_mut`，读 `*g` 未支持）。
  - typecheck `check_expr/mod.rs` `ExprKind::Assign` 处理器对 `*guard` 左值特判：guard 有 `deref_mut` 时生成 `*(guard.deref_mut()) = v`（`HirExpr::DerefSet`，base 为 `deref_mut` 调用返回的 `&mut T`），复用既有 DerefSet→DerefWrite 路径；borrowck/regionck 对 `DerefSet` 仅 `check_expr(base/value)`，方法调用 base 兼容。
  - 验证 `tests/run-pass/guard_deref_mut.rl`：MutexGuard `*g=99` / `println(*g)` / `m.value` 写回生效 + RwLockWriteGuard `*wg=33` / `println(*wg)`，作用域自动解锁无死锁。

### Y4c（✅ 已完成，lang-defects #8 / P7c，2026-08-29）
- `Channel<T>` 泛型化（元素不再限 i64）+ `SendError<T>`/`RecvError`/`TryRecvError` 错误类型 + `bounded_channel(capacity)` 有界队列 + `Arc<LockFreeQueue>`。
- **探测（2026-08-28）**：`Channel<T> { queue: Vec<T> }` 构造触发复合字段推断障碍（`Vec<T>` 无法从空 `Vec::new()` 推断 T；Y4a 仅裸泛型字段）且无 turbofish（`Channel::<i64>::new()` 语法错误）。登记 [`lang-defects.md`](./lang-defects.md) #8 待专项（复合字段推断 + 泛型返回类型推断）。
- **落地（2026-08-29，P7c / Y4c）**：`Channel<T>`/`Sender<T>`/`Receiver<T>`/`RecvAsync<T>` 已泛型化，`channel::<T>()` 以 turbofish 显式指定元素类型（MVP 无复合字段推断，靠 turbofish 兜底）；`send`/`recv` 收发 `T` 值（`recv` 返回 `Result<T, IoError>`）。验收 `tests/run-pass/{channel,recv_async,async_combo}.rl` 用 `channel::<i64>()` 收发值通过，`future.rl` 内部 `Channel<i64>` 通道驱动异步 fd 事件；173 用例全绿。详见 lang-defects #8。
- **Y4c 收尾（2026-08-30）：`bounded_channel(capacity)` 有界队列完成**——`Channel<T>` 增 `capacity: i64` 字段（`channel::<T>()` 置 0 无界；新增 `bounded_channel::<T>(capacity)` 置 >0 有界）；`send` 满则经 `Condvar::wait` 阻塞（消费者腾出空间后 `recv`/`try_recv` 经 `notify_one` 唤醒发送者，提供背压）；`try_send` 有界满返回 `false`；验证 `tests/run-pass/bounded_channel.rl`。
- **Y4c 收尾（2026-08-30）：通道错误类型完成**——新增 `SendError<T>`/`RecvError`/`TryRecvError` 三个类型 + `Result` 语义方法 `send_result`（关闭返回 `Err(SendError{val})`）/`recv_result`（关闭且空返回 `Err(RecvError)`）/`try_recv_result`（空 `Err(kind:0)`、断开 `Err(kind:1)`）；既有 `recv`/`try_recv`/`send`（`Option`/`()`）便捷 API 保留以兼容 173 用例，未破坏回归；验证 `tests/run-pass/channel_errors.rl`。
- **Y4c 收尾（2026-08-30）：`Arc<LockFreeQueue>` 类型层完成**——`Sender<T>`/`Receiver<T>`/`RecvAsync<T>` 内部共享句柄由 `Rc<Channel<T>>` 改为 `Arc<Channel<T>>`（K3 原子引用计数，`Arc::new`/`clone` 经 typecheck 内建支持），`Channel<T>` 可在多线程间安全共享引用计数；**限制**：将 `Sender`/`Receiver` *移动* 进 `Thread::start(fn() -> i64)` 派生的线程受「无参入口」限制（线程 API 暂不接收值实参），故跨线程移动场景待线程传参能力补齐，单线程/同线程共享已完全可用，run-pass 套件（含 channel/bounded_channel/channel_errors/recv_async/async_combo）全绿。

## 验证

`y4a_generic_struct.{rl,out}`（泛型推断构造）+ `y4b_deref.{rl,out}`（Deref 解引用分派）+ `y4b_mutex_generic.{rl,out}`（泛型 Mutex 原型，164 用例全绿）；`tests/run-pass/mutex_value.rl`（std `Mutex<T>` 携值锁 + get/get_mut，lang-defects #6 验收）+ `tests/run-pass/{channel,recv_async,async_combo}.rl`（`Channel<T>` 泛型收发，lang-defects #8 验收），173 用例全绿；`mutex_generic.{rlyeh,out}`（泛型 Mutex + Deref guard + bounded Channel + 错误类型，Y4b-3/4 后续）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
| 2026-08-28 | 风险探测（泛型 struct 字面量构造障碍）+ 拆分 Y4a/Y4b/Y4c |
| 2026-08-28 | Y4a 泛型 struct 字面量构造推断完成（162 用例全绿） |
| 2026-08-28 | Y4b-1 Deref 解引用分派完成 + 细化 Y4b-2/3/4（163 用例全绿） |
| 2026-08-28 | Y4b-2 泛型 impl 静态方法推断完成 + 泛型 Mutex 原型（164 用例全绿；std 迁移待专项） |
| 2026-08-28 | Y4c 探测（复合字段推断 + 无 turbofish 障碍）登记 lang-defects #8，暂缓 |
| 2026-08-29 | Y4b-2 std Mutex 泛型化（lang-defects #6 / P5）完成：`sync/module.rl` 携值锁 + get/get_mut，mutex_value.rl 验收，173 用例全绿 |
| 2026-08-29 | Y4c Channel<T> 泛型化（lang-defects #8 / P7c）完成：turbofish 兜底 + 收发 T 值，channel/recv_async/async_combo 验收，173 用例全绿 |
| 2026-08-30 | Y4b-3 `RwLock<T>` 泛型化 + `RwLockReadGuard`/`RwLockWriteGuard`（Deref 分发 + desugar 识别 read_guard/write_guard）完成：`rwlock_value.rl` 验证，全量套件通过 |
| 2026-08-30 | Y4b-4 guard 可变访问 DerefMut 完成：`*guard = x` 经 `deref_mut` 分发（`MutexGuard`/`RwLockWriteGuard` 增 `deref_mut`，`MutexGuard` 补 `deref`），`guard_deref_mut.rl` 验证 |
| 2026-08-30 | Y4c 收尾：`bounded_channel(capacity)` 有界队列完成（`Channel<T>` 增 `capacity` + `send` 满阻塞 + `recv`/`try_recv` 唤醒发送者 + `try_send` 满返 false），`bounded_channel.rl` 验证 |
| 2026-08-30 | Y4c 收尾：通道错误类型 SendError/RecvError/TryRecvError 完成（新增 `recv_result`/`try_recv_result`/`send_result` Result 方法，既有 Option/() API 保留兼容），`channel_errors.rl` 验证 |
| 2026-08-30 | Y4c 收尾：`Arc<LockFreeQueue>` 类型层完成（`Channel<T>` 共享句柄 Rc→Arc，原子引用计数跨线程安全共享），run-pass 套件全绿 |
