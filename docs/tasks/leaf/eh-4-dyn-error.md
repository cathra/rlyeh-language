# EH-4 泛型错误类型 `DynError`（anyhow 式便捷）

> **级别**：P2 · **风险**：🟠 中 · **状态**：🟢 部分完成（M1 / M2 ✅ 2026-09-21；M3 用户侧落地） · **归属**：0.2.0-AA
> **索引**：[`../rfc/error-handling.md`](../rfc/error-handling.md) §4.4 · **关联**：`eh-2-error-source.md`、`m2a-error-protocol.md`

## 目标
提供统一的错误抽象，降低库间错误类型摩擦：
- `DynError = Box<dyn Error>` 等价类型（或带所有权的 `OwningError`）。
- `Result<T>` 别名（= `Result<T, DynError>`）。
- 便捷构造宏（如 `bail!`/`ensure!`）。

## 现状（2026-09-21 实测）
- 此前仅有具体错误类型 `IoError`/`TimeoutError`；`dyn Error` 可作**局部变量**与 `&dyn Error`（`Error::source` 链），但 `Box<dyn Error>` **不可构造**（`Box::new(<dyn 值>)` 报「该类型不支持堆装箱」），且 `Box<dyn P>` 接收者的方法调用会读越界槽。

## 风险分解与落地
- **M1（中）`DynError` 类型定义 + `Box<dyn Error>` 装箱/拆箱** ✅
  - 形态：`rlyeh-std/rlyeh/io/error.rl` 的 `struct DynError { inner: Box<dyn Error> }`（**非类型别名**）。
  - 构造：`IoError::into_dyn()`（`Box::new(self)` → `&*b` 取堆对象引用 → `let d: dyn Error = r` 构造 2 槽胖指针 → `Box::new(d)` 装箱）、`DynError::from_io(e)`。
  - 转发：`DynError::message()` / `source()` 经内层 `Box<dyn Error>` **虚调用**分派；`impl DynError: Error` 使自身可参与背链并上转为 `&dyn Error`。
  - 表示：`Box<dyn Error>` = 指向 2 槽胖指针 `{data, vtable}` 的对象指针，`data` 指向**堆上拥有**的具体错误（由 `Box::new` 装箱）。
- **M2（低）`Result<T>` 别名 + 库函数默认返回 `DynError`** ✅
  - 用户侧 `type LoadResult<T> = Result<T, DynError>;`（泛型类型别名早已支持）；`?` 在同错类型上下文直接传播。
  - 注：别名**不能**命名为 `Result`（与内建 `Result<T, E>` 枚举同名冲突风险），文档建议 `LoadResult` / `IoResult` 等前缀名。
- **M3（低）`bail!`/`ensure!` 宏** ⚠️ **用户侧落地**
  - std 无法提供：宏注册表按**编译单元**隔离，std 模块文件里的 `macro_rules!` 对用户代码不可见（实测「未定义的宏 `ensure`」）。
  - 可用形态（`tests/run-pass/eh_dyn_error.rl` 内定义）：
    ```rlyeh
    macro_rules! bail {
        ($($t:tt)*) => { return Result::Err($($t)*) };
    }
    macro_rules! ensure {
        ($cond:expr, $($t:tt)*) => {
            if !($cond) { return Result::Err($($t)*) }
        };
    }
    ```
  - **宏系统缺口（本轮实测，待专项）**：① 宏不可从 std / 模块文件导出（无 `pub macro` 或等价机制）；② `$e:expr` **不匹配含 `::` 的路径调用**（`bail!(IoError::from_kind(..))` 报「规则均不匹配」）→ 须用 `$($t:tt)*` 重复（`tt` 是唯一能吞下 `::` 的元变量）；③ transcriber 须展开为**单个表达式**（`return Result::Err(..)` 不能带结尾分号，否则报「展开产物含多余 token」）；④ 元变量展开优先级不高于一元 `!`，条件须写 `!($cond)`（否则报 `expected bool, found i64`）。

## 已修复的编译器缺口（2 处，2026-09-21）
1. **`dyn Protocol` 的槽数**（`crates/rlyeh-typecheck/src/check_expr/field.rs::type_slot_count`）：此前未列 `Type::Dyn` 分支，落 `_ => Err(不支持堆装箱)`，致 `Box::new(<dyn 值>)`（`Box<dyn Protocol>` 的唯一构造入口）不可用。现返回 `Ok(2)`（槽 0 = data 指针、槽 1 = vtable 指针），与胖指针布局一致。
2. **堆包裹 protocol 对象的虚调用**（`crates/rlyeh-typecheck/src/check_expr/method/builtin.rs`）：
   - ① dyn 接收者识别新增 `heap_wrapper_inner(recv_ty)` 分支（`Box`/`Rc`/`Arc`/`Gc` 包裹的 `dyn Protocol`）——此前只认裸 `dyn P` 与 `&dyn P`，`Box<dyn P>` 会落到常规 impl 分派并报 `FunctionNotFound`；
   - ② 该分支内先 `heap_ptr_hir(recv_hir, recv_ty)` 解出槽 0 的**堆对象指针**（`Box<T>` 局部为 1 槽聚合，槽 0 指向堆对象；对非堆包裹类型为恒等，裸 `dyn` / `&dyn` 行为不变），再按「指针指向 2 槽胖指针」读槽 0/1 构造 `CallIndirect`。

## 未纳入（后续）
- **`dyn Error + Send + Sync`**：auto-protocol 约束在 `dyn` 侧尚未参与约束求解，跨线程 `DynError` 暂不校验。
- **析构**：MVP 下外层箱释放不回收内层箱（沿用 `Box` 既有约定，见 `tests/run-pass/box_leak.rl`）。
- **`DynError` 作为类型别名**（对齐 Rust 形态）：受「`type` 仅根单元收集」与「`dyn` 只接受单段名」两条限制，暂以结构体形态落地。
- **`anyhow!` 风格格式化构造**：需 `format!` 与宏系统能力扩展。

## 受影响组件
`rlyeh-std`（`io/error.rl` 新增 `DynError` / `IoError::into_dyn`；`io/module.rl`、`rlyeh/module.rl` 重导出）；`rlyeh-typecheck`（`check_expr/field.rs`、`check_expr/method/builtin.rs`）。

## 验证
- `tests/run-pass/eh_dyn_error.rl`（+ `.out`，8 行）：`into_dyn()` 构造 + `message()` 虚调用（`boom`）、`source()` 转发（`Option<&dyn Error>` → `is_none`）、`&dyn Error` 上转后 `message()`、`Result<T, DynError>` 成功 / `ensure!` 失败 / `bail!` 失败（`match` 取 `e.message()` → `entity not found`）、`?` 传播（成功 `4` / 失败 `is_err`）。
- 全量 `rlyeh test tests`：**341/341 通过**（本轮新增 1 例）。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-20 | 评审通过后建叶子 |
| 2026-09-21 | **M1+M2 落地**：`struct DynError { inner: Box<dyn Error> }` + `IoError::into_dyn()` / `DynError::from_io()` + `message()`/`source()` 转发 + `impl DynError: Error`；连带修复 2 处编译器缺口（`type_slot_count` 对 `dyn` 返回 2 槽；`check_method_call` 识别堆包裹 protocol 对象并解出堆对象指针）。**M3** 实测宏注册表按编译单元隔离（std 宏不可导出）→ `bail!`/`ensure!` 改**用户侧**落地并登记宏系统 4 项约束。新增 `tests/run-pass/eh_dyn_error.{rl,out}`；全量 341/341 |
