# 切片类型系统（规划）

> **状态**：规划中（未实现）
> **定位**：类型系统增强；与定长数组 `[T; N]` 互补，解锁运行时长度未知的连续序列视图
> **关联任务**：Y1（二进制文件 IO 切片签名升级路径见 §6）

**叶子任务文档**：[`slice-s1-type-layer.md`](leaf/slice-s1-type-layer.md) · [`slice-s2-codegen.md`](leaf/slice-s2-codegen.md) · [`slice-s4-tests-docs.md`](leaf/slice-s4-tests-docs.md)

## 1. 目标

提供运行时长度未知的连续序列「视图」：

- 切片引用 `&[T]`（只读）/ `&mut [T]`（可变）——胖指针（data 指针 + 长度）。
- 与定长数组 `[T; N]` 互补：`[T; N]` 长度编译期已知，`[T]` 长度运行时已知。
- 解锁 `File::read(&mut [u8])` / `File::write(&[u8])` 等场景（见 Y1 升级路径 §6）。

## 2. 现状与卡点

| 层 | 现状 |
|----|------|
| 解析 | `parser/src/ty.rs::parse_array_type` 已支持 `[T]`（`size=None`）与 `[T; N]`，产出 `AstType::Array(inner, Option<size>)` |
| 类型 | `typecheck/src/types.rs::Type` 仅有 `Array(Box<Type>, usize)`（定长），**无切片变体** |
| resolve | **唯一卡点**：`check_expr/resolve.rs:174-195` 在 `size=None` 时直接 `Err(Unsupported "数组类型缺少大小")` |
| codegen | `Type::Array` 当前按 `FieldScalar::Ptr`（指针）存储，`field_scalar_of` 判为 Ptr；长度编码在类型中（编译期已知） |
| 动态切片 | `Vec`/`String` 取子切片 `x[lo..<hi]` 已部分支持（产出 `&str` 时走 `StrFat` 双槽胖指针），可作为切片引用实现的参考 |

## 3. 语法设计

```
&[T]       // 只读切片引用（胖指针：data ptr + len）
&mut [T]   // 可变切片引用
[T]        // 裸切片类型（DST，无大小）——MVP 仅作为 &[T] 的"元素类型"，不可单独作值/参数
```

- **切片构造（desugar）**：
  - 数组/`Vec`/`String` 取切片：`arr[lo..<hi]` / `arr[lo...hi]` / `arr[lo<..hi]`，产出 `&[T]` 胖指针（与现有动态切片机制同源）。
  - 全切片：`&arr[..]`、`&s[..]`。
- **字面子串**：`s[lo..<hi]` 已支持 `&str`（`StrFat`），`&s[..]` 同理复用。
- MVP 暂不允许 `[T]` 作为 `let` 值类型（DST 必须经由 `&`/`&mut`/裸指针出现，与 Rust 一致）。

## 4. 类型规则

- 新增 `Type::Slice(Box<Type>)`（建议独立变体，表达清晰、与 `Array` 解耦）。
- `resolve_ast_type`：`AstType::Array(inner, None)` → `Type::Slice(inner)`（**取消原报错**）；`Some(n)` → `Type::Array(inner, n)` 不变。
- `&[T]` 解析路径：`&` 已支持嵌套 `parse_type`，`[T]` 经上述 resolve 为 `Slice` 后被 `Ref` 包裹为 `Ref(Slice(T), Immutable/Mutable)`。
- `compatible_with`：切片与切片按元素类型兼容（复用 `Ref` 的可变性规则：可变可传只读）；`&[T]` ↔ `&[U]` 当 `T`/`U` 兼容。
- 函数参数/返回支持 `&[T]` / `&mut [T]`（解锁 Y1）。
- 索引 `slice[i]`、`.len()`、`slice[lo..<hi]`（再切片，产出子切片）的类型与越界 clamp 规则与现有数组/`Vec` 切片一致。

## 5. codegen 设计

- **切片引用 = 胖指针（2 槽）**，对齐 `&str` 的 `StrFat`（data 指针 + 长度）：槽 0 = data 指针（T*），槽 1 = 长度（usize）。
- `rlyeh_hir::FieldScalar` 新增 `SliceFat`（或泛化复用 `StrFat`）；`field_scalar_of` 对 `Type::Slice` 返回该胖指针种类。
- **再切片** `[lo..<hi]`：计算 `data + lo*sizeof(T)` + 长度 `hi-lo`，零拷贝产出子切片胖指针。
- 动态切片已存在（`Vec`/`String`），切片引用复用同一套 desugar（产出胖指针）。
- 越界 clamp 到 `[0, len)`（与现有数组/`Vec` 切片一致）。
- 裸切片 `[T]`（DST）在 MVP 不允许独立存储，故无需 DST 尺寸推导。

## 6. 标准库与 Y1 升级路径

- Y1 降级方案（`read_bytes`/`write_bytes` 用 `Vec<u8>` 缓冲）在切片落地后可平滑升级：
  - 新增 `File::read(&mut [u8]) -> usize` / `File::write(&[u8]) -> usize`（二进制安全）。
  - 在 Y1 叶子文档中标注 `Vec<u8>` 版为过渡方案，切片版为首选。
- 切片方法 protocol（MVP 后，复用 V3 Iterator 适配器）：`len`、`iter`、`split_at`、`first`/`last` 等。

## 7. 验收标准

- [ ] `&[i64]` / `&mut [i64]` 可作函数参数与返回类型。
- [ ] 数组/`Vec`/`String` 取切片 `&x[lo..<hi]` 产出正确胖指针（data + len）。
- [ ] 切片索引 `s[i]`、再切片 `s[a..<b]`、`.len()` 正确。
- [ ] 越界 clamp 与现有数组/`Vec` 行为一致。
- [ ] Y1 `File::read(&mut [u8])` / `write(&[u8])` 二进制安全可用。
- [ ] `tests/run-pass` 切片用例全绿，无回归。

## 8. 任务拆分（分阶段）

- **S1 类型层**
  - S1a：`Type` 新增 `Slice(Box<Type>)`；`Display` 实现 `[T]`（引用场景显示 `&[T]`）。
  - S1b：`resolve.rs` 取消 `None` 报错，改为 `Type::Slice`；`compatible_with` / `field_scalar_of` 适配。
  - S1c：`parser` 无需改（`[T]` 已解析，`&` 已支持嵌套）；补切片索引/再切片 expr 的类型推导接线。
- **S2 codegen 层**：`FieldScalar` 新增 `SliceFat`；切片胖指针布局 + 索引/再切片/`.len()` 代码生成；越界 clamp。
- **S3 标准库接线**：`core.rl` 切片方法 + `File::read`/`write` 切片重载（Y1 升级）。
- **S4 测试与文档**：`tests/run-pass` 切片用例；`grammar.md`/`semantics.md` 补切片章节；Y1 叶子文档更新升级说明。

## 9. 风险与开放问题

- 切片与现有 `&str` 视图的互操作（如 `&[u8]` ↔ `&str` 视角转换）是否 MVP 提供——标注为开放问题。
- 多维切片 / 切片嵌套（切片的元素是切片）规划中留白。
- 裸切片 `[T]` 作为独立值的 DST 支持（未来阶段，本规划 MVP 不覆盖）。
