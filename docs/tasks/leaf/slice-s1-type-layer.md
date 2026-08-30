# S1 切片类型层

> **所属**：切片类型系统（规划 [`slice-type-system.md`](../slice-type-system.md)）
> **状态**：✅ 已完成（2026-08-30；`&[T]`/`&mut [T]` 在类型层可表示，resolve 不再报错）
> **阶段**：S1（类型层）

## 目标

新增 `Type::Slice(Box<Type>)`，取消 `resolve.rs` 对 `[T]`（无大小）的报错，打通切片在类型层的表示。

## 技术细节

- `crates/rlyeh-typecheck/src/types.rs`：`enum Type` 新增 `Slice(Box<Type>)`；`Display` 增加 `Slice(t) => write!(f, "[{t}]")`（引用场景经 `Ref` 显示 `&[T]`）。
- `crates/rlyeh-typecheck/src/check_expr/resolve.rs:174`：`AstType::Array(inner, None)` 改为 `Ok(Type::Slice(inner))`，不再 `Err(Unsupported)`；`Some(n)` 路径不变。
- `compatible_with` / `field_scalar_of` / `type_mono_key` 已有 `_` 兜底，`Slice` 暂落 `Ptr`/默认分支，无需改。
- 索引/再切片 expr 类型推导接线属 S1c（见规划 §8），本叶子不含。

## 验收

- [ ] `[T]` resolve 为 `Type::Slice(T)` 而非报错。
- [ ] `&[T]` / `&mut [T]` 在类型层可表示（`Ref(Slice(T), ..)`）。
- [ ] `cargo check -p rlyeh-typecheck` 通过（穷尽匹配补全）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-30 | 由切片规划细化为叶子 |
| 2026-08-30 | 实现：`types.rs` 新增 `Type::Slice` + `Display`；`resolve.rs` 取消 `[T]` 报错改为 `Type::Slice`；`cargo check` 全工作区零错误；探针 `&[i64]`/`&mut [i64]` 参数类型通过类型检查与编译（RC=0）。`Slice` 暂经 `field_scalar_of` 兜底映射为指针，真胖指针待 S2 |
