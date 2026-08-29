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
- **where 子句 + `Into` blanket 已支持（P6b / P6c-1/2，2026-08-29）**：`where` 子句语法已可解析进 AST（约束名记录，bound 支持 `::` 路径与泛型实参）；`Into::into` 的 blanket 语义由 typechecker 实现——`Into::<U>::into(x)` 约束求解确认 `impl From<A_source> for U_target` 存在后改写 `From::from(x)`（不注册真实 blanket impl，规避其方法体 `From::from(self)` 泛型静态检查障碍）。**仍待专项**：通用 `where` 约束求解（任意 blanket impl 的方法体在泛型层面静态检查 + 关联类型投影依赖 bound）尚未实现，`where` 约束目前仅记录不校验。
- **修复方向**：parser 泛型类型实参支持 `::` 路径 + 增加 `where` 子句（`?` 运算符 From 自动转换前提）。

### 5. 泛型 impl 静态方法类型参数推断（源自 Y4b）— ✅ 已修复（Y4b-2）

- **症状**：`MyMutex::new(42)`（泛型 impl `impl<T> MyMutex<T> { fn new(v: T) }`）→ typecheck 报 `泛型 impl MyMutex 的静态方法 new`（method.rs MVP 不支持——self 类型无法确定类型参数）。
- **影响**：Y4b-2 的 `Mutex<T>::new(v)` 泛型构造不可用。
- **修复（2026-08-28，Y4b-2）**：method.rs 静态方法分支对泛型 impl 从实参推断类型参数（裸 `Generic(tp)` 参数直接填充）。
- **遗留限制**：复合参数（`Vec<T>` 等）的统一推断暂不覆盖。

### 6. std `Mutex` 泛型化（源自 Y4b-2）— ✅ 已完成（Y4b-2，P5，2026-08-29）

**现状**：`sync/module.rl` 的 `Mutex<T> { p: i64, value: T }` 已为「带值锁」——`Mutex::new(v: T)` 携带数据，`MutexGuard<T> { p: i64, value: &mut T }` 经裸指针访问 `value`（MVP 生命周期退化，绕过借用检查）；`get`/`get_mut` 已落地（P5，2026-08-29）。

**目标签名**：
```rlyeh
struct Mutex<T> { p: i64, value: T }
struct MutexGuard<T> { p: i64, value: *mut T }   // 或 value: &mut T（生命周期 MVP 退化）
impl<T> Mutex<T> {
    fn new(v: T) -> Mutex<T>;
    fn lock_guard(self) -> MutexGuard<T>;   // 加锁 + 捕获 value 指针
}
impl<T> MutexGuard<T> { fn get(&self) -> &T; fn get_mut(&mut self) -> &mut T; }
```
即 `Mutex<T>` 从「空锁」升级为「带值锁」，用户可 `Mutex::new(42).lock_guard()` 持锁访问数据。

**语言级障碍**（按严重度）：
- **生命周期/借用缺失**：`lock_guard()` 返回携带 `&mut value` 的守卫，需要借用检查器理解「锁生命周期 = 守卫生命周期」，Rlyeh MVP 无借用生命周期；MVP 退化用**裸指针**（`value: i64` 存 `&value` 地址，`get/get_mut` 经 `__rlyeh_deref`/extern 间接访问，绕过借用检查）。
- **Deref trait 分派（可选）**：若用户期望 `*g` 直接解引用取值（而非显式 `.get()`），需语言级 Deref trait 分派（Y4b 已识别为破坏性大改动）。MVP 可只用显式 `get/get_mut` 方法绕过。
- **泛型静态方法携带数据构造**：`new(v: T)` 的 `Mutex { p, value: v }` 构造——字段 `value: T` 为裸 `Generic(tp)`，Y4a 的字段推断已支持（直接推断，非复合）。

**波及范围（破坏性改动，需统一迁移）**：
- `sync/module.rl`：`Mutex`/`MutexGuard` 泛型化 + `new(v)` + `lock_guard` 返回值改造 + `get/get_mut` 新增
- `core.rl`：`import sync::Mutex`/`MutexGuard` 泛型化
- `rlyeh-desugar/src/guard.rs`：`lock_guard` 注入机制按方法名特判（AST 阶段无类型信息，**无需改**；但注入的 `unlock()` 语义保持）
- driver `sync_test.rs`：所有 `Mutex::new()` → `Mutex::new(载荷)`，`m.lock()`/`try_lock()` 保持
- `mutex_guard.rl` 及 06-sync 目录示例、tests/run-pass 下所有用到 `Mutex::new()` 的文件

**风险**：中-高（数据载荷生命周期 + 泛型构造波及面大）。**MVP 最小集**：先只改 `Mutex<T>` 携带值 + `get/get_mut` 裸指针访问（不动 Deref 分派），signature 落地即可验证。

**落地（2026-08-29，P5 / Y4b-2）**：`sync/module.rl` 的 `Mutex<T> { p: i64, value: T }`、`MutexGuard<T> { p: i64, value: &mut T }` 已实现；`new(v: T)`/`lock_guard`/`get`/`get_mut` 落地。验收 `tests/run-pass/mutex_value.rl`（`Mutex::new(42)` → `get()`=42 → `(&mut g).get_mut()`+`*p=100` → `get()`=100，输出 `42/100/0`）；`mutex_guard.rl` 验证 guard 自动解锁。全量 173 用例全绿。

### 7. 完整 Poller kqueue 分派（源自 Y2b）— ✅ 已完成（Y2c，2026-08-29）

**现状**：kqueue 基础设施**已全部就绪**（Y2a/Y2b）：
- driver `platform_ir.rs` 已注入 `__rlyeh_kqueue`/`__rlyeh_kevent`（macOS=2/BSD=4 原生转发 `kqueue`/`kevent`，其他平台 stub -1）
- `io/nio.rl` 已有 `kevent_make`（32B 结构体）、`kqueue_new`、`kevent_ctl`（EV_ADD/DELETE）、`kevent_wait`（阻塞等待），`y2b_kqueue_wait.rl` 已验证等待真实 socketpair 事件
- `Poller`（`io/nio.rl`）已按 `__rlyeh_target_os()` 分派：macOS/BSD 建 kqueue（`kq>0`）走 EV_ADD/kq_poll，其他平台 `kq=-1` 回退 poll(2)；新增 `tests/run-pass/y2c_poller_kqueue.rl` 端到端验证（socketpair 一端写 → `(&mut q).register(fd0, 7, io::nio::Interest::Readable)` → `q.poll(-1)` 返回就绪事件 `token==7` 且 `is_readable()`，输出 `3`）

**目标**：`Poller` 结构加 `kq: i64` 字段，`new`/`register`/`deregister`/`poll` 按 `__rlyeh_target_os()` 分派：
- macOS/BSD（码 2/4）：`new` 建 kq + 注册表；`register` = EV_ADD + `kevent_ctl`；`deregister` = EV_DELETE；`poll` = `kevent_wait`（O(1)，非 O(n) 扫描）
- 其他平台（Linux/Windows/WASI）：回退 poll(2) 兜底（现有实现）
- WASI 下现有 `poll`/`fcntl` 短路返回 Err（保留）

**关键点**：**无语言级障碍**（纯 std 改写 + 平台注入已存在），是三者中**最低风险**项。只需改 `io/nio.rl` 的 `Poller` struct + 方法；`future.rl` 内部对 `Poller::new()/register()/poll()` 的调用**签名不变**，无需改动。

**波及范围（破坏性改动）**：
- `io/nio.rl`：`Poller` 加 `kq` 字段 + 四方法分派改造
- `y2b_kqueue_wait.rl`：可直接复用现有 kqueue 原语（不变）；新增 Poller 集成测试
- `y2a_kqueue.rl`：验证 FFI 绑定（不变）
- examples/projects/chatd/{client,server}.rl、smoke/main.rl：`Poller::new()/register/poll` 调用签名不变，**无需迁移**（纯内部实现替换）
- core.rl import、future.rl：签名不变，无改动

**风险**：低。**推进顺序建议**：#7 已完成（Y2c）；随后做 #6（Mutex，MVP 裸指针集），最后 #8（触发多重语言级障碍，需专项）。

### 8. `Channel<T>` 泛型化（源自 Y4c）— ✅ 已完成（Y4c，P7c，2026-08-29）

**现状**：`sync/module.rl` 的 `Channel<T> { queue: Vec<T> }` 元素已参数化 `T`；`Sender<T>`/`Receiver<T>`/`RecvAsync<T>` 同理；`recv`/`try_recv`/`next` 返回 `Result<T, IoError>`（close 且空返回 `Err`），`recv_async` 的 `Future::Output = T`（P7c，2026-08-29）。

**目标**：`Channel<T>`/`Sender<T>`/`Receiver<T>`/`RecvAsync<T>` 泛型化，元素类型参数化。

**语言级障碍**（双重，均登记于 Y4 系列）：
- **复合字段 `Vec<T>` 推断缺失**：构造 `Channel { queue: Vec::with_capacity(8) }` 时 `Vec<T>` 的 T 无法从字段实参推断（Y4a 仅支持裸 `Generic(tp)` 字段直接推断，复合 `Vec<T>` 未覆盖）；`let ch: Channel<i64> = Channel { ... }` 报 `expected Channel<i64>, found Channel`（构造返回类型无实参）。
- **无 turbofish**：`Channel::<i64>::new()` 语法错误，无法显式指定泛型实参；泛型函数返回类型 `fn channel<T>() -> Channel<T>` 的 T 无法从返回注解/调用上下文推断。
- **`RecvAsync<T>` 的 `Future::Output` 投影**：`type Output = i64` → `type Output = T`，关联类型投影在泛型上（`F::Output` 已可用，future.rl join_all 已验证），但 Channel 的 `poll` 返回 `Poll<T>` 需 T 可构造（`Poll::Ready(v)` v: T 从队列元素来，OK）。

**波及范围（破坏性改动）**：
- `sync/module.rl`：`Channel`/`Sender`/`Receiver`/`RecvAsync`/`ChannelPair` 全部泛型化 + `channel<T>()` + `recv/recv_async` 签名 `T`
- `recv_async.rl`、tests/run-pass/channel.rl、tests/run-pass/recv_async.rl、examples/std-demos/06-sync/channel.rl：类型注解/推导迁移
- `SendError<T>`/`RecvError` 错误类型（目标签名，当前 Channel 无 send 错误返回）
- core.rl import 泛型化

**风险**：高（复合字段推断 + 无 turbofish 双重语言级障碍，破坏性大）。**建议**：登记 lang-defects 待专项（Y4c 已登记）；先修复合字段推断或引入 turbofish/泛型返回推断后再推进。

**落地（2026-08-29，P7c / Y4c）**：`Channel<T>`/`Sender<T>`/`Receiver<T>` 已泛型化，`channel::<T>()` 以 turbofish 显式指定元素类型（MVP 无复合字段推断，靠 turbofish 兜底）；`send`/`recv` 收发 `T` 值（`recv` 返回 `Result<T, IoError>`）。验收 `tests/run-pass/{channel,recv_async,async_combo}.rl` 用 `channel::<i64>()` 收发值通过；`future.rl` 内部 `Channel<i64>` 通道驱动异步 fd 事件。全量 173 用例全绿。

### 9. 方法返回 `&self.struct_field` 引用悬空（源自 P7d-1 / Y6c）

- **症状**：方法返回 `&self.field`（取结构体字段地址作为引用返回）时，codegen 生成的字段槽地址指向**按值传入的 `self` 参数**（方法返回后该参数所在槽销毁），调用方拿到的引用悬空——解引用读到垃圾值或段错误。
- **实证（2026-08-29，P7d-1）**：`trait Error { fn source(&self) -> Option<&dyn Error> }` 的 `impl Error for AppError { fn source(&self) -> Option<&dyn Error> { let d: &dyn Error = &self.source; ... } }`（`source: &IoFailure` 为引用字段）输出错乱；改为「持有引用类型字段 + 上转**存储的引用值**」（`let d: &dyn Error = self.source;`）则正确（`self.source` 是字段里的引用值，上转而非取地址）。
- **根因**：V1「真实取址」（`&obj.field` → GEP 槽地址）对**局部/按值 self 字段**生成的是临时栈地址，返回该地址即悬空；MVP 借用检查未将「返回字段地址」判定为 `DanglingReference`（仅对返回 `&x` 局部变量/绑定引用变量判定）。
- **影响**：错误包装器（及其他需返回内部引用者）**不能**用 `&self.field` 形式返回字段引用；须把内部引用作为**引用类型字段**存储，并返回该字段的引用**值**（上转型），而非对字段取地址。
- **约束/规避**：MVP 暂不支持 `&dyn Error` 直接作 struct 字段（dtor/vtable 槽归零），故 `source` 链载体退化到具体引用类型（`&IoFailure`）；待借用检查补全「返回字段地址 = 悬空」判定 + `&dyn Error` 字段支持后解除。
- **状态**：📋 登记（MVP 已知限制，非阻塞——用引用值字段规避即可，见 y6-error-source.md P7d-1 小节）。

### 10. 跨模块 `&mut self` 方法自动借用失效（源自 Y2c）

- **症状**：用户程序跨模块调用 `io::nio::Poller` 的 `&mut self` 方法 `q.register(...)` → typecheck 报 `expected (), found io::nio::Poller`；同模块 `future.rl` 调用 `q.register(...)` 正常（W3 executor 已落地）。
- **对照**：跨模块调用 `&self` 方法（如 `q.poll(0)`）正常；仅 `&mut self` 自动借用跨模块失败。
- **绕过**：显式 `(&mut q).register(...)` 提供 `&mut` 接收者，功能正确（y2c_poller_kqueue.rl 已验证）。
- **影响**：std 跨模块 `&mut self` API（`Poller::register`/`reregister`/`deregister`）从用户代码直接调用报错，需显式 `&mut` 借用；属 API 人体工学名实非阻塞。
- **状态**：📋 登记（MVP 已知限制，非阻塞）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-28 | 建立专项文档；登记泛型 struct 字面量构造障碍（Y4）+ `&dyn Error` 构造障碍（Y6） |
| 2026-08-28 | 登记泛型 trait 实参路径不支持 + where 子句缺失（Y6b） |
| 2026-08-28 | 登记泛型 impl 静态方法推断（Y4b-2，已修复）+ std Mutex 泛型化待专项 |
| 2026-08-28 | 登记完整 Poller kqueue 分派待专项（Y2b） |
| 2026-08-28 | 登记 Channel<T> 泛型化待专项（Y4c，复合字段推断 + 无 turbofish） |
| 2026-08-28 | 细化 #6（Mutex<T> 空锁→带值锁，生命周期 MVP 裸指针集 + Deref 可选）；#7（Poller kqueue 分派，基础设施已齐、无语言障碍、低风险优先）；#8（Channel<T> 复合字段推断 + 无 turbofish 双重障碍） |
| 2026-08-29 | 新增 #9（方法返回 `&self.struct_field` 引用悬空，源自 P7d-1 / Y6c 实证）：V1 真实取址对按值 self 字段生成临时栈地址、返回即悬空，MVP 借用检查未判定为 DanglingReference；规避——内部引用用引用类型字段存储并返回引用值（非取地址） |
| 2026-08-29 | #7 完整 Poller kqueue 分派标记已完成（Y2c，y2c_poller_kqueue.rl 端到端验证 macOS kqueue EV_ADD/kq_poll）；新增 #10（跨模块 &mut self 自动借用失效，源自 Y2c） |
| 2026-08-29 | #6 std Mutex 泛型化（Y4b-2，P5）与 #8 Channel<T> 泛型化（Y4c，P7c）标记已完成（代码早已落地，本轮补 mutex_value.rl 验证 + 文档收尾）；173 用例全绿 |
