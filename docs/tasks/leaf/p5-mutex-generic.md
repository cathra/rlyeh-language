# P5 std Mutex 泛型化

> **所属专项**：[专项开发计划](../专项开发计划.md)（P5）
> **来源缺陷**：[`leaf/lang-defects.md`](lang-defects.md) #6（std `Mutex` 泛型化，源自 Y4b-2）
> **状态**：✅ 已完成（2026-08-29：完整泛型 `Mutex<T>` 带值锁 + `MutexGuard<T>` 引用访问——此前「泛型 struct 引用字段构造」障碍已由 **P7a 的 `unify` 复合/引用字段推断**修复）
> **风险**：中（原「中-高」——已通过 P5a-1~P5d-2 拆分为可独立实现/验证的子步骤；生命周期 MVP 用裸指针方案 + P5b-3 无悬垂验证隔离风险）
> **前置能力**：P4（生命周期/借用 MVP 思路）；Y4a/Y4b（泛型构造 + 静态方法推断已齐）

## 目标

`Mutex<T>` 从「空锁」升级为「带值锁」+ `MutexGuard<T>` 持值访问，用户可 `Mutex::new(42).lock_guard()` 持锁访问数据。

## 现状（2026-08-28）

- `sync/module.rl` 的 `Mutex { p: i64 }` 为空锁（仅 pthread 原语指针，无数据载荷），`Mutex::new()` 无参
- `MutexGuard { p: i64 }` 仅持锁指针、`unlock()` 走 extern，无数据访问能力
- 语言级能力已齐（Y4a 泛型构造 + Y4b-1 Deref + Y4b-2 泛型静态方法推断），原型验证通过

## 目标签名（MVP 最小集）

```rlyeh
struct Mutex<T> { p: i64, value: T }
struct MutexGuard<T> { p: i64, value: *mut T }   // 或 value: &mut T（生命周期 MVP 退化）
impl<T> Mutex<T> {
    fn new(v: T) -> Mutex<T>;
    fn lock_guard(self) -> MutexGuard<T>;   // 加锁 + 捕获 value 指针
}
impl<T> MutexGuard<T> { fn get(&self) -> &T; fn get_mut(&mut self) -> &mut T; }
```

**MVP 决策**：`get/get_mut` 用**裸指针**（`value: i64` 存 `&value` 地址，经 `__rlyeh_deref`/extern 间接访问，绕过借用检查）；**不做 Deref trait 分派**（Y4b 识别为破坏性大改动，MVP 用显式方法绕过）。

## 语言级障碍

- **生命周期/借用缺失**：`lock_guard()` 返回携带 `&mut value` 的守卫需借用检查器理解「锁生命周期 = 守卫生命周期」，MVP 用裸指针绕过
- **Deref trait 分派（可选）**：`*g` 解引用需语言级 Deref，MVP 用显式 `get/get_mut`
- 泛型静态方法携带数据构造：`new(v: T)` 的 `Mutex { p, value: v }`，字段 `value: T` 为裸 `Generic(tp)`，Y4a 字段推断已支持

## 波及范围（破坏性改动，需统一迁移）

- `sync/module.rl`：`Mutex`/`MutexGuard` 泛型化 + `new(v)` + `lock_guard` 返回值改造 + `get/get_mut` 新增
- `core.rl`：`import sync::Mutex`/`MutexGuard` 泛型化
- `rlyeh-desugar/src/guard.rs`：`lock_guard` 注入按方法名特判（AST 阶段无类型信息，**无需改**）
- driver `sync_test.rs`：`Mutex::new()` → `Mutex::new(载荷)`
- `mutex_guard.rl` 及 06-sync 目录示例、tests/run-pass 下所有 `Mutex::new()` 文件

## 执行步骤（拆分）

- **P5a（中风险）`Mutex<T>` 带值锁 + `new(v)`**
  - P5a-1（中风险）：`sync/module.rl` `struct Mutex<T> { p: i64, value: T }` + `impl<T> Mutex<T> { fn new(v: T) -> Mutex<T> }`（`Mutex { p: <libc 初始化>, value: v }`）；`core.rl` import 泛型化。验收：`Mutex::new(42)` 编译，锁原语方法（lock/unlock/try_lock）保持。
  - P5a-2（低风险）：确认 `new` 的 `value: T` 字段为裸 `Generic(tp)`，Y4a 字段推断正常；`Mutex::new(vec![1,2])` 复合元素不误判。验收：`Mutex::new(42)`/`Mutex::new("s")` 均可用。

- **P5b（中风险）`MutexGuard<T>` 持值访问（裸指针生命周期正确性）**
  - 关键风险：`lock_guard()` 返回守卫捕获 `&value` 地址（存 i64 裸指针），`get/get_mut` 间接访问——**地址生命周期必须与守卫一致**，且守卫必须在 `Mutex` 值仍存活时使用（`lock_guard(self)` 消费 `Mutex`，无悬垂风险，但需确认 desugar 注入的 unlock 时序）。
  - P5b-1（中风险）：`MutexGuard<T>` 加 `value: i64` 字段（存 `&self.value` 地址）；`lock_guard(self)` 返回守卫时 `value: <取地址>`。验收：守卫结构体携带地址字段。
  - P5b-2（中风险）：`get(&self) -> &T`/`get_mut(&mut self) -> &mut T` 经 `__rlyeh_deref`/extern 按地址读值（`__rlyeh_load_i64`/`__rlyeh_store_i64` 间接）。验收：`Mutex::new(42).lock_guard().get()` 取到 42；`get_mut` 写回生效。
  - P5b-3（中风险）正确性验证：**无悬垂**——确认 desugar 的 `lock_guard` 块尾自动 `unlock()` 与守卫读取顺序（先 get 后 unlock）；`get_mut` 后 `Mutex` 值内存仍有效（`lock_guard(self)` 已消费 Mutex，值随守卫存活）。验收：守卫块内读写正常、块尾解锁后无 panic/未定义行为。
- **P5c（中风险）desugar guard 注入确认**
  - P5c-1（中风险）：确认 `rlyeh-desugar/src/guard.rs` 的 `lock_guard` 按方法名特判（AST 阶段无类型信息），泛型化后 `Mutex<T>` 方法名仍为 `lock_guard`，注入 `unlock()` 语义不变。验收：guard 块尾自动解锁仍生效。
  - P5c-2（低风险）：确认 `lock_guard` 返回的守卫携带值指针后，desugar 注入的块尾 unlock 不与守卫读取冲突（unlock 在守卫作用域结束后）。验收：`{ let g = m.lock_guard(); g.get(); }` 块尾自动解锁。
- **P5d（低风险）测试/示例迁移**
  - P5d-1（低风险）：driver `sync_test.rs` 所有 `Mutex::new()` → `Mutex::new(载荷)`（标量测试用 `0`/`42`）。验收：sync_test 编译 + 运行通过。
  - P5d-2（低风险）：`mutex_guard.rl`/06-sync 示例/run-pass 全量迁移（`Mutex::new(初始值)` + 断言 get 值）。验收：迁移后回归全绿。

## 验收标准（整体）

- `Mutex::new(42).lock_guard()` 持锁经 `get` 取值
- 全量迁移后回归全绿

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-28 | 由专项开发计划 P5 生成叶子文档（拆分 P5a/b/c/d） |
| 2026-08-28 | 细化 P5a→P5a-1/2、P5b→P5b-1/2/3（裸指针生命周期正确性 + get_mut 写回验证）、P5c→P5c-1/2、P5d→P5d-1/2，全部到可执行子步骤粒度 |
| 2026-08-28 | 整体与 P5b 风险降为中（子步骤均为中/低，裸指针生命周期由 P5b-3 无悬垂验证隔离） |
| 2026-08-28 | ✅ 部分完成：`sync/module.rl` 带值锁 MVP——`Mutex { p, value: i64 }` + `new(v: i64)` + `lock(&self)`/`unlock(&self)`/`try_lock(&self)`；`MutexGuard { p, value_ptr: &mut i64 }` + `get`/`get_mut` 引用访问（`lock_guard(&mut self)` 借调用方 self 规避悬垂）；`Channel.m` 用 `Mutex::new(0)` 纯锁；`sync_test.rs` 迁移 `Mutex::new()` → `Mutex::new(0)` + 新增 `mutex_value_lock_guard`（get/get_mut 验证）。验证：`Mutex::new(42).lock_guard().get()` 读、`*g.get_mut()=77` 写回、函数尾自动解锁；sync_test 7 全过 + cargo test 全绿。**障碍**：完整泛型 `Mutex<T>`/`MutexGuard<T>` 因「泛型 struct 引用字段构造」报 `Guard.value 期望 &mut T, found &mut i64`（Y4a 泛型 struct 构造的字段类型替换未覆盖 `&mut T` 引用字段）——登记待专项 |
| 2026-08-29 | ✅ 障碍解除，升级为**完整泛型**：P7a 把 `construct.rs` 的字段推断改为 `unify` 递归统一后，泛型 struct 引用字段构造（`Guard { value: &mut self.value }`）可用。`sync/module.rl` 最终为 `Mutex<T> { p, value: T }` + `new(v: T)` + `lock_guard(&mut self) -> MutexGuard<T>`；`MutexGuard<T> { p, value: &mut T }` + `get`/`get_mut`；`Condvar::wait<T>(m: Mutex<T>)`；`Channel.m: Mutex<i64>`（纯锁用 `Mutex::new(0)`）。验证：`Mutex::new(true)`（**非 i64 类型，证明泛型生效**）+ `lock_guard().get()` → 1；`*g.get_mut()=99` 写回 → `m2.value`=99；sync_test 7 全过 + cargo test 全绿 |
