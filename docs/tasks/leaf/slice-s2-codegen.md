# S2 切片 codegen

> **所属**：切片类型系统（规划 [`slice-type-system.md`](../slice-type-system.md)）
> **状态**：✅ 已完成（2026-08-30；胖指针 / unsize coercion / 索引 / 再切片 / clamp / `.len()` / `&mut [T]` 写入均验证通过）
> **阶段**：S2（codegen 层）

## 目标

切片引用的胖指针（data 指针 + 长度，2 槽）代码生成，以及对齐 `&str` 的 `StrFat` 布局。

## 技术细节

- `rlyeh-hir/src/lib.rs`：`FieldScalar` 新增 `SliceFat`（泛化复用 `StrFat` 思路）；`field_scalar_of` 对 `Type::Slice` 返回该胖指针种类。
- `Type → LirType` 映射：`Slice` → 胖指针 `LirType`（2 槽）；索引 `s[i]`、再切片 `s[a..<b]`、`.len()` 经 `StrFat` 同源逻辑生成（data + len）。
- 越界 clamp 到 `[0, len)`（与现有数组/`Vec` 切片一致）。
- codegen 基于 `LirType` 派发，新增 `SliceFat` 不影响现有 `Type` 匹配（已确认 `rlyeh-codegen` 零处引用 `Type::*` 变体）。

## 验收

- [x] `&[i64]` / `&mut [i64]` 生成正确胖指针（data + len）。
- [x] 切片 `.len()` 代码生成正确。
- [x] 切片索引 `s[i]`（`&mut [T]` 的 `s[i] = v` 写入亦通）。
- [x] 再切片 `s[a..<b]`。
- [x] 越界 clamp（下界 0、上界 `len`、且保证 `en >= st`）。
- [x] `cargo build` 通过；`rlyeh test` 184 用例全通过（零回归）。

## 已实现（2026-08-30）

| 位置 | 改动 |
|------|------|
| `rlyeh-hir`/`rlyeh-lir` | `FieldScalar::SliceFat` / `LirType::SliceFat`（布局 `{i8*, i64}`，与 `StrFat` 同构） |
| `rlyeh-lir/src/lower/mod.rs` | `field_scalar_to_lir`；参数类型跨函数传播支持 `SliceFat`（原仅 `StrFat`） |
| `rlyeh-codegen/src/llvm/llvm_util.rs` | `field_scalar_llvm` / `field_scalar_lir` / `llvm_type` / `builtin_fmt` 四处补 `SliceFat` |
| `rlyeh-codegen/src/llvm/llvm_ctor.rs` | by_value 对象排除逻辑纳入 `SliceFat`（同 `StrFat`，避免污染 data 槽） |
| `rlyeh-typecheck/src/types.rs` | `field_scalar_of`：`Ref(Slice(_))` → `SliceFat`；`compatible_with` 新增 unsize coercion `&[T; N]` → `&[T]` |
| `rlyeh-typecheck/src/check_expr/call.rs` | `make_slice_fat()` 构造胖指针（`Alloc{slots:2, by_value, is_strfat}` + 两次 `FieldSet`）；用户函数调用实参处插入 |
| `rlyeh-typecheck/src/check_expr/method.rs` | `check_method_call` 对切片接收者特判 `.len()`（读槽 1；切片在 `core.rl` 无 impl） |
| `rlyeh-typecheck/src/check_expr/index_enum.rs` | `check_index` 新增 `Type::Slice` 分支：取槽 0 的 data 指针后按元素步长 GEP（`&[u8]` → 字节步长） |
| `rlyeh-typecheck/src/check_expr/field.rs` | `check_slice` 新增切片再切片分支：`{data + lo*sizeof(T), clamp(hi) - lo}` 零拷贝子区间视图 |

## 关键坑（后续接手必读）

1. **两条参数检查路径**：`check_call` 中 `builtin_signature` 分支只处理内建函数；用户函数走
   `lookup_fn_signature` 分支（文件尾部）。unsize coercion 必须插在**后者**，插错位置静默失效。
2. **LIR 无参数类型信息**：参数默认 `i64`，靠调用点实参反向传播。若被调函数被内联，调用点
   `Call` 消失 → 传播不发生；此时正确性由内联保证，但函数的独立 IR 定义参数仍是错的（死代码）。
   验证时必须用**不会被内联**的函数体（如含 `while` 循环）才能测到真实的参数传递。
3. **main 返回值不映射 exit code**：`fn main() -> i64 { 5 }` 运行退出码仍为 0。验证请用
   `println(...)`（与 `tests/run-pass` 用例一致）。
4. `--force` 对 `run` 子命令无效（缓存仍命中）；强制重编译请用 `--cache-dir <新目录>`。
5. **LIR 的 `Assign` 不传播类型**：`let tmp = <胖指针变量>` 会把 `tmp` 落成默认 `i64` 槽
   （IR 出现 `alloca i64` 却 `store { i8*, i64 }`），运行期直接崩溃。构造胖指针时**不要**
   用 `let` 绑定胖指针中间值——直接内联接收者表达式；只可绑定 `i64` / `Ptr` 这类默认值
   恰好正确的中间量（`len` / `start` / `end` / `data` / `ptr` 均安全）。
6. **`PtrAdd` 的 1 字节步长仅当 `elem == FieldScalar::Str`**（`rlyeh-mir/src/lower.rs:565`）。
   `&[u8]` 的字节偏移必须传 `Str`，传 `Int` 会按 8 字节步进。
7. **`[u8; N]` 数组非紧凑存储（既有行为，非切片引入）**：`let bs: [u8; 4] = [1,2,3,4]`
   直接索引求和得 `1` 而非 `10`。`&[u8]` 继承底层数组该行为；需要紧凑字节缓冲请用
   `Vec<u8>`（Y1 的二进制缓冲即走 `Vec<u8>`）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-30 | 由切片规划细化为叶子 |
| 2026-08-30 | 实现：胖指针类型表示（`SliceFat`）+ unsize coercion + `.len()`；探针验证 `define i64 @total({ i8*, i64 } %xs)` 与 `call @total({i8*, i64})`，跨函数（非内联）传参与 `.len()` 均正确；184 用例零回归 |
| 2026-08-30 | 实现：切片索引 `s[i]`（含 `&mut [T]` 写入）、再切片 `s[a..<b]`（零拷贝 + 边界 clamp 到 `[0, len]`）；验证 `sum(&xs)=150`、`xs[1..<3]` 求和 `=50`、上界越界 clamp `=120`、`&mut` 写入 `=9`；184 用例零回归 |
