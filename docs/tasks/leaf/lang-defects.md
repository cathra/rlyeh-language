# 语言级缺陷跟踪专项（Lang Defects）

> **所属阶段**：阶段 Y（专项跟踪文档，非独立阶段）
> **状态**：📋 待办（登记探测到的语言级类型系统/泛型/dyn 缺陷，统一评估后修复）
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

集中跟踪 Rlyeh 语言级缺陷（类型系统、泛型、dyn trait 对象、切片参数化等），按规则：**探测到的新语言级缺陷先记录到本文档**，评估影响后再统一修复。区别于 [`parser-rework.md`](./parser-rework.md)（内建解析器重构专项）。

## 背景

Y 阶段任务风险评估（2026-08-28）在实测中探测到多个语言级缺陷，阻碍 Y4/Y6/Y1 等任务的目标签名落地。

## 已登记缺陷

### 1. 泛型 struct 字面量构造无法实例化（源自 Y4）— ✅ 已修复（Y4a）

- **症状**：`struct MutexGuard<T> { value: T }` + `let g: MutexGuard<i64> = MutexGuard { value: 42 };` → typecheck 报 `argument 0 of struct MutexGuard field value expects T, found i64`。
- **即使显式类型注解 `MutexGuard<i64>` 仍失败**——`T` 未被替换为具体类型。
- **影响**：Y4 的 `Mutex<T>`/`MutexGuard<T>` 泛型锁、`Channel<T>` 泛型化的**字面量构造路径**不可用。std 泛型类型（`Vec<T>`/`Result<T,E>`/`HashMap<K,V>`）经内建/特判构造，用户自定义泛型 struct 缺泛型实例化支持。
- **修复（2026-08-28，Y4a）**：construct.rs `check_struct_construct` 在 `type_args` 为空且 struct 含泛型参数时，从字段实参推断（字段类型裸 `Generic(tp)` 的直接推断）。
- **遗留限制**：复合字段（`Vec<T>`/`HashMap<K,V>`）的统一推断暂不覆盖。

### 2. `&dyn Error` trait 对象构造/上转型失败（源自 Y6）

- **症状**：
  - `let d: dyn Error = e;`（e: MyError）→ `expected dyn Error, found MyError`（无自动上转型）
  - `let d: &dyn Error = r;`（r: &MyError）→ `expected &dyn Error, found &MyError`（引用也不能转 dyn）
- **影响**：Y6 的 `source() -> Option<&dyn Error>` 目标签名虽可声明，但无法构造非 `None` 的 `&dyn Error` 值（链式 source 语义不可实现）。
- **修复方向**：支持具体类型 → dyn trait 对象的上转型（`&T as &dyn Trait` / 自动 coercion）。

### 3.（预留）切片参数化（U1，源自 Y1）

- `read(&mut [u8])`/`write(&[u8])` 切片实参依赖 U1 切片成熟——`&mut [u8]`/`&[u8]` 作函数参数 + 切片值传递，待确认语言支持后登记。

### 4. 泛型 trait 实参不支持路径 + 无 where 子句（源自 Y6b）

- **症状**：`impl From<io::error::IoErrorKind> for IoError` → parser 报 `expected '>', found Colon`（泛型类型实参不支持 `::` 路径）；裸名 `impl From<IoErrorKind>` 可行。
- **影响**：Y6b 的 `From`/`Into` std 层只能对**裸名**类型生效；模块内路径类型（`io::error::IoErrorKind`）的泛型 impl 不可写。
- **where 子句缺失**：`impl<T, U> Into<U> for T where U: From<T>`（blanket impl）不可行——无 `where` 子句语法。
- **修复方向**：parser 泛型类型实参支持 `::` 路径 + 增加 `where` 子句（`?` 运算符 From 自动转换前提）。

### 5. 泛型 impl 静态方法类型参数推断（源自 Y4b）— ✅ 已修复（Y4b-2）

- **症状**：`MyMutex::new(42)`（泛型 impl `impl<T> MyMutex<T> { fn new(v: T) }`）→ typecheck 报 `泛型 impl MyMutex 的静态方法 new`（method.rs MVP 不支持——self 类型无法确定类型参数）。
- **影响**：Y4b-2 的 `Mutex<T>::new(v)` 泛型构造不可用。
- **修复（2026-08-28，Y4b-2）**：method.rs 静态方法分支对泛型 impl 从实参推断类型参数（裸 `Generic(tp)` 参数直接填充）。
- **遗留限制**：复合参数（`Vec<T>` 等）的统一推断暂不覆盖。

### 6.（待专项）std `Mutex` 泛型化（源自 Y4b-2）

- 语言级能力已齐（Y4a 泛型构造 + Y4b-1 Deref + Y4b-2 泛型静态方法推断），原型验证通过。
- std sync/module.rl 的 `Mutex` 泛型化为**破坏性改动**（波及 core.rl、guard.rs desugar、driver sync_test.rs、mutex_guard.rl 等），需统一迁移。

### 7.（待专项）完整 Poller kqueue 分派（源自 Y2b）

- kqueue 核心能力已验证（`kevent_wait` 等待真实 socketpair 事件，Y2b）。
- `Poller` 结构加 `kq` 字段 + `new`/`register`/`deregister`/`poll` 按 `__rlyeh_target_os()` 分派 kqueue（macOS）vs poll（兜底）vs epoll（Linux）为**破坏性改动**（波及 core.rl/future.rl/examples/tests 等 31 处），需统一迁移。

### 8.（待专项）`Channel<T>` 泛型化（源自 Y4c）

- **症状**：`struct Channel<T> { queue: Vec<T> }` 泛型化后，构造 `Channel { queue: Vec::new() }` 的复合字段 `Vec<T>` 无法推断 T（Y4a 仅支持裸 `Generic(tp)` 字段推断）；`let ch: Channel<i64> = Channel { ... }` 报 `expected Channel<i64>, found Channel`（构造返回类型无实参）。且 Rlyeh **无 turbofish**（`Channel::<i64>::new()` 语法错误）。
- **影响**：`Channel<T>`/`Sender<T>`/`Receiver<T>`/`RecvAsync<T>` 泛型化 + `SendError<T>`/`RecvError` 错误类型不可落地。
- **修复方向**：扩展复合字段推断（`Vec<T>` 从元素/上下文）+ 泛型函数/静态方法返回类型推断（或 turbofish 语法）。登记待专项。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-28 | 建立专项文档；登记泛型 struct 字面量构造障碍（Y4）+ `&dyn Error` 构造障碍（Y6） |
| 2026-08-28 | 登记泛型 trait 实参路径不支持 + where 子句缺失（Y6b） |
| 2026-08-28 | 登记泛型 impl 静态方法推断（Y4b-2，已修复）+ std Mutex 泛型化待专项 |
| 2026-08-28 | 登记完整 Poller kqueue 分派待专项（Y2b） |
| 2026-08-28 | 登记 Channel<T> 泛型化待专项（Y4c，复合字段推断 + 无 turbofish） |
