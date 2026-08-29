# P7 Channel\<T\> 泛型化

> **所属专项**：[专项开发计划](../专项开发计划.md)（P7）
> **来源缺陷**：[`leaf/lang-defects.md`](lang-defects.md) #8（`Channel<T>` 泛型化，源自 Y4c）
> **状态**：✅ 基本完成（2026-08-29：P7a 复合字段推断 + P7b turbofish + **P7c std `Channel<T>` 全类型泛型化**（含 `RecvAsync<T>` 的 `Output = Option<T>`，已验证 `Channel<bool>` 非 i64 元素）；P7d 错误类型暂不实施——见下）
> **风险**：中（原「高」——已通过 P7a-1~P7d-2 拆分为可独立实现/验证的子步骤 + 两个语言级障碍（复合字段推断/turbofish）隔离为前置子任务 P7a/P7b 逐个化解；P7b 方案 B 已否决，消除架构性推断高风险）
> **前置能力**：P7a 复合字段推断 + P7b turbofish/泛型返回推断

## 目标

`Channel<T>`/`Sender<T>`/`Receiver<T>`/`RecvAsync<T>` 泛型化，元素类型参数化。

## 现状（2026-08-28）

- `sync/module.rl` 的 `Channel { queue: Vec<i64> }` 元素**硬编码 i64**；`Sender`/`Receiver`/`RecvAsync` 同理
- `recv`/`try_recv`/`next` 返回 `Option<i64>`，`recv_async` 的 `Future::Output = i64`（close 且空返回哨兵 `-1` 表达 `Option::None`）

## 语言级障碍（双重，均登记于 Y4 系列）

1. **复合字段 `Vec<T>` 推断缺失**：构造 `Channel { queue: Vec::with_capacity(8) }` 时 `Vec<T>` 的 T 无法从字段实参推断（Y4a 仅支持裸 `Generic(tp)` 字段直接推断，复合 `Vec<T>` 未覆盖）；`let ch: Channel<i64> = Channel { ... }` 报 `expected Channel<i64>, found Channel`（构造返回类型无实参）
2. **无 turbofish**：`Channel::<i64>::new()` 语法错误，无法显式指定泛型实参；泛型函数返回类型 `fn channel<T>() -> Channel<T>` 的 T 无法从返回注解/调用上下文推断
3. **`RecvAsync<T>` 的 `Future::Output` 投影**：`type Output = i64` → `type Output = T`，关联类型投影在泛型上（`F::Output` 已可用，future.rl join_all 已验证）

## 技术方案（拆分，前置障碍与 std 改造分开）

### P7a（中风险）复合字段推断扩展

`construct.rs:36-47` Y4a 推断仅匹配 `Type::Generic(tp)`（裸 `T`），扩展匹配复合形态 `Type::Named("Vec", [Generic(tp)])`（`Vec<T>` 从元素实参推断）与 `HashMap<K,V>`（从键值实参推断）。

- **P7a-1（中风险）`Vec<T>` 复合字段推断**：construct.rs 匹配 `Type::Named("Vec", [Generic(tp)])`，从字段实参 `Vec::with_capacity(8)` 的**元素类型**（或 `vec![x]` 的 x 类型）推断 T。验收：`Channel { queue: Vec::with_capacity(8) }` 构造可推断 T。
- **P7a-2（中风险）`HashMap<K,V>` 复合字段推断**：匹配 `Type::Named("HashMap", [Generic(k), Generic(v)])`，从键值实参推断。验收：`HashMap::with_capacity(8)` 字段推断 K/V。
- **P7a-3（中风险）构造返回类型实参传播**：`let ch: Channel<i64> = Channel { ... }` 构造返回类型带实参，字段推断结果**反向传播**到构造类型（消除 `expected Channel<i64>, found Channel`）。验收：`let ch: Channel<i64> = Channel { queue: ... };` 编译通过。
- **P7a-4（低风险）回归**：既有裸 `Generic(tp)` 字段推断不受影响。验收：Y4a 用例 + 166 用例回归全绿。

- **涉及**：`check_expr/construct.rs:36-47`
- **验收**：`Channel { queue: Vec::with_capacity(8) }` 构造可推断 T；`let ch: Channel<i64> = Channel { ... }` 报错消除

### P7b（中风险）turbofish 语法或泛型返回推断

**现状澄清（2026-08-28 代码定位）**：
- **自由函数 turbofish 已支持**：`mod.rs:67-103` `parse_expr_prec` 中 `check_turbofish()` → `ExprKind::Call { type_args }`，支持 `json.parse::<T>(s)`、`foo::<T1,T2>(args)`、嵌套 `>>` 拆分（`control.rs:189-199` 已验证）
- **关联路径段间 turbofish 未支持**：`primary.rs:196-201` 路径解析遇到 `::<` 即停止（留给 Call 后缀）；`Channel::<i64>::new()` 的 `::<i64>::` 中间还有个 `::new` 路径段，Call 后缀无法处理——`<i64>` 后接 `::new` 而非 `(args)`，**当前会解析失败**
- 泛型函数返回类型推断：`fn channel<T>() -> Channel<T>` 的 T 需从 let 注解/调用上下文推断，当前类型检查无此能力

**方案评估（二选一，已选定方案 A；方案 B 已否决）**：

**方案 A：关联路径段间 turbofish（`Channel::<i64>::new()`）【已选定】**
- **改动点**：`primary.rs` 路径解析在 `::<` 时需支持「turbofish 后继续路径段」→ `Type::method` 关联函数调用（`ExprKind::Call` 的 callee 含 type_args + path）
- **难度**：中（parser 路径解析 + typecheck 关联函数泛型实参实例化）
- **风险**：中（语法糖改动，需确保与现有 `foo::<T>(args)` 不冲突——区分 `::<T>::` 与 `::<T>(`；P7b-3 冲突回归覆盖）
- **优点**：显式、无类型推断歧义；与 std `Vec::new`/`HashMap::with_capacity` 的路径调用机制天然契合（`macro_.rs:186-206` 已生成这类路径）
- **缺点**：需改 parser + typecheck 泛型实例化两处

**方案 B：泛型函数返回类型从上下文推断【已否决，不执行】**
- **改动点**：typecheck 推断 `fn channel<T>() -> Channel<T>` 的 T——从 let 注解 `let ch: Channel<i64> = channel();` 反推，或从实参
- **难度**：高（需要「期望类型驱动」的泛型实例化，与现有「构造表达式自上而下」推断机制冲突，Y4a 的字段推断也是局部自下而上）
- **风险**：~~高~~（引入期望类型传播，影响整个类型推断架构）——**因选用方案 A，此高风险路径不实施**
- **优点**：无需新语法
- **缺点**：架构性改动大，T 可能无法从上下文确定（如 `channel()` 无注解时）

**建议**：**选方案 A**（改动更局部、无架构性推断改动，且与 std 路径调用机制契合）。MVP 可退化为：仅支持「路径段间 turbofish → 关联函数调用」，不做通用期望类型推断。

**细化子步骤（方案 A）**：
- **P7b-1（中风险）parser 路径段间 turbofish**：`primary.rs` 路径解析支持 `::<T1,T2>::`（turbofish 后继续路径段），生成含 type_args 的路径 callee。验收：`Channel::<i64>::new` 可解析为 AST（不报错）。
- **P7b-2（中风险）typecheck 关联函数泛型实例化**：`ExprKind::Call` 的 callee 为带 type_args 的类型路径时，实例化关联函数（如 `Channel::<i64>::new` → `new` 返回 `Channel<i64>`）。验收：`Channel::<i64>::new()` 返回 `Channel<i64>`。
- **P7b-3（低风险）冲突回归**：确认 `foo::<T>(args)` 自由函数 turbofish 不受影响（区分 `::<T>::` 与 `::<T>(`）。验收：`json.parse::<HashMap<i64,Vec<i64>>>(s)` 回归全绿。

- **涉及**：`crates/rlyeh-parser/src/expr/primary.rs` + `expr/mod.rs` + typecheck 关联函数调用
- **验收**：`channel::<i64>()` 或 `let ch: Channel<i64> = channel();` 可用（方案 A 下为 `Channel::<i64>::new()` 可写）

### P7c（中风险）`sync/module.rl` 全类型泛型化

`Channel`/`Sender`/`Receiver`/`RecvAsync`/`ChannelPair` 泛型化 + `channel<T>()`；`recv`/`try_recv`/`next` 返回 `Option<T>`、`recv_async` 的 `Future::Output = T`（去掉哨兵 -1 语义，改 `Option<T>` 投影）。

- **P7c-1（中风险）`Channel<T>`/`Sender<T>`/`Receiver<T>` 泛型化**：`Channel { queue: Vec<T> }` + `Sender<T>`/`Receiver<T>` 元素参数化；`channel<T>()` 工厂。验收：`Channel<i64>`/`Channel<String>` 构造 + `send`/`recv` 编译。
- **P7c-2（中风险）`RecvAsync<T>` + `Future::Output = T`**：`RecvAsync<T>` 的 `type Output = T` 投影；去掉哨兵 -1 语义，close 且空时 `next` 返回 `None`。验收：`recv_async` Output = T，close 空返回 None。
- **P7c-3（低风险）`core.rl` import 泛型化**：`import sync::{Channel, Sender, Receiver, RecvAsync, ChannelPair}` 泛型化。验收：import 后泛型类型可用。

- **涉及**：`sync/module.rl`、`core.rl` import
- **验收**：`Channel<i64>`/`Channel<String>` 构造/发送/接收泛型化

### P7d 错误类型 + 测试迁移

- **P7d-2（低风险）测试迁移** ✅ 已完成：`recv_async.rl`/`channel.rl`/`async_combo.rl`/`examples/.../06-sync/channel.rl` 迁移（`channel::<i64>()` turbofish + `sync::RecvAsync<i64>` + `Output = Option<T>` 的 match 解包）。验收：4 个文件运行通过 + cargo test 全绿。
- **P7d-1 错误类型** ⏸️ **暂不实施**（记录决策）：`SendError<T>`（携带发送的值）/`RecvError` 属**目标签名**——当前 `Channel<T>` 为**无界队列**，`send` 恒成功（无失败路径），引入 `SendError<T>` 需改 `send` 签名返回 `Result<(), SendError<T>>` 并再次全量迁移所有调用点，收益为 0（无实际错误场景）。**待有界通道 / 断开检测（`Sender` 全 drop 后 send 失败）落地时一并引入**，届时 `SendError<T>` 携带值可用于错误恢复。
- **涉及**：`sync/module.rl` 错误类型 + tests/examples
- **验收**：迁移后回归全绿（已达成）；错误类型待有界通道时补

## 执行步骤

1. P7a-1→P7a-4：construct.rs 复合字段推断扩展 + 构造返回实参传播
2. P7b-1→P7b-3：parser 路径段间 turbofish + typecheck 关联函数实例化
3. P7c-1→P7c-3：`sync/module.rl` 全类型泛型化 + `channel<T>()` + recv 系列签名
4. P7d-1→P7d-2：测试/示例迁移；`SendError<T>`/`RecvError` 错误类型

## 验收标准（整体）

- `Channel<i64>` 构造/发送/接收泛型化
- `recv_async` Output = T
- 回归全绿

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-28 | 由专项开发计划 P7 生成叶子文档（拆分 P7a/b/c/d，定位 construct.rs:36-47） |
| 2026-08-28 | 细化 P7a→P7a-1/2/3/4（Vec/HashMap 复合推断 + 构造返回实参传播）、P7b→P7b-1/2/3（根因：自由函数 turbofish 已支持 mod.rs:67-103，关联路径段间未支持，选方案 A）、P7c→P7c-1/2/3、P7d→P7d-1/2，全部到可执行子步骤粒度 |
| 2026-08-29 | ✅ P7a 完成：`construct.rs` 泛型 struct 构造的字段类型推断从「裸 `Generic(tp)` 直接匹配」改为 **`unify` 递归统一**（支持复合字段 `Vec<T>`/`HashMap<K,V>`）+ 按 `def.type_params` 顺序提取实参。验证 `let b: Bag<i64> = Bag { items: Vec::with_capacity(8) }` 推断 T=i64（此前报 `expected Bag<i64>, found Bag`）。✅ P7b 完成：P7b-1 parser（`expr/mod.rs`）turbofish 关闭后支持继续 `::` 路径段（`Type::<T>::method`）；P7b-2 typecheck（`call.rs` turbofish 调用优先走 static method call；`method.rs check_static_method_call` + `generic.rs check_generic_call` 接收 type_args 预填 subst）。验证 `Vec::<i64>::new()`、`Cell::<i64>::new(42)`、`Bag::<i64>::new()` + `b.add(1)` 全通；自由函数 turbofish `json::parse::<i64>`/`::<HashMap<i64,i64>>` 无回归；cargo test 全绿 |
| 2026-08-29 | ✅ P7c 完成：`sync/module.rl` 全类型泛型化——`Channel<T>`/`Sender<T>`/`Receiver<T>`/`ChannelPair<T>`/`RecvAsync<T>`；`channel<T>()`（turbofish 指定元素类型）+ `send(val: T)`/`recv()`/`try_recv()`/`next()` → `Option<T>`；`RecvAsync<T>` 的 `Future::Output = Option<T>`（**替代 MVP 哨兵 -1**）；`Condvar::wait<T>` 已泛型化。构造处显式带实参（`sync::Channel<T>{..}`/`Sender<T>`/`Receiver<T>`/`ChannelPair<T>`/`RecvAsync<T>`）——Rc 嵌套时字段推断无法反推外层 T。验证：`channel::<i64>()` send/recv → 42；`channel::<bool>()`（**非 i64 元素**）→ 1；`block_on(&mut RecvAsync<i64>)` → `Some(7)` / close 空 → `None`。迁移 4 个文件（recv_async.rl/channel.rl/async_combo.rl/examples 06-sync/channel.rl）+ cargo test 全绿。P7d-1 `SendError<T>`/`RecvError` 暂不实施（无界队列 send 恒成功，待有界通道时引入） |
