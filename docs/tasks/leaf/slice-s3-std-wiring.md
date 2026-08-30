# S3 切片标准库接线

> **所属**：切片类型系统（规划 [`slice-type-system.md`](../slice-type-system.md)）
> **状态**：🔧 进行中（2026-08-30；切片方法 `len`/`first`/`last`/`iter` + `Vec→切片` 视图完成；`File` 重载与 `split_at` 待办）
> **阶段**：S3（标准库接线）

## 目标

`core.rl` 切片方法 + `File::read`/`write` 切片重载（Y1 升级路径）。

## 技术细节

- `core.rl` 切片方法 trait（复用 V3 Iterator 适配器）：`len`、`iter`、`split_at`、`first`/`last`。
- Y1 升级：`File::read(&mut [u8]) -> usize` / `File::write(&[u8]) -> usize`（二进制安全）；原 `Vec<u8>` 版 `read_bytes`/`write_bytes` 标为过渡方案（见 Y1 叶子文档）。
- `&[u8]` ↔ `&str` 视角转换（开放问题，标注）。

## 验收

- [x] 切片方法 `len` / `first` / `last` / `iter` 可用。
- [ ] `File::read(&mut [u8])` / `write(&[u8])` 二进制安全可用（前提已备，见下）。
- [ ] `split_at`（返回切片二元组；MVP 元组返回待评估）。
- [ ] Y1 叶子文档更新升级说明（待 File 重载落地后）。
- [x] `rlyeh test` 184 用例零回归。

## 已实现（2026-08-30）

| 位置 | 改动 |
|------|------|
| `rlyeh-typecheck/src/check_expr/method.rs` | 切片内建方法：`len()`（读槽 1）、`first()`/`last()`（`s[0]` / `s[len-1]`）、`iter()`（零拷贝构造 `IterRef<T>`：槽 0=data、槽 1=len） |
| `rlyeh-typecheck/src/check_expr/method.rs` | `Vec<T>` 的 `as_slice()` / `as_mut_slice()` → `&[T]` / `&mut [T]` 零拷贝视图（Vec 槽 0=data、槽 1=len） |
| `rlyeh-typecheck/src/check_expr/index_enum.rs` | 修复 `Vec<u8>` 索引步长：原固定 8 字节，与紧凑字节存储不符；现按 `U8` 走 1 字节（与数组 / 切片一致） |

## 关键发现

1. **`Vec<u8>` 紧凑、`[u8; N]` 非紧凑**（实测）：`Vec<u8>` 为 1 字节/元素
   （`sum_bytes(v.as_slice()) == 10`）；`[u8; N]` 数组为 8 字节/元素（直接索引求和只得 `1`）。
   故**字节类切片必须从 `Vec<u8>` 取**——这印证 Y1 降级方案选 `Vec<u8>` 并非权宜，而是必需。
2. **Y1 升级路径的前提已具备**：`v.as_mut_slice()` 提供零拷贝 `&mut [u8]`，实测写入真实
   反映到 Vec 缓冲（`1,2,3 → 2,3,4`）。下一步只需在 `core.rl` 增 `File::read/write` 重载，
   取切片 data 指针 + len 调 `fread`/`fwrite`（Y1 的 `read_bytes`/`write_bytes` IR 转发可复用）。
3. **切片方法一律走 typecheck 特判**（`core.rl` 无法为 `[T]` 写 impl），与 S2 的 `.len()` 一致；
   `iter()` 需绕过 `IterRef::new` 的 `RawPtr` 实参检查（切片槽 0 是 `Ptr`），按同布局直接落槽。
4. `&[u8]` ↔ `&str` 视角转换仍为开放问题，未实现。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-30 | 由切片规划细化为叶子 |
| 2026-08-30 | 实现：切片方法 `first`/`last`/`iter`（`len` 已于 S2 完成）、`Vec<T>` 的 `as_slice`/`as_mut_slice` 零拷贝切片视图；修复 `Vec<u8>` 索引步长 bug；验证 `sum(&xs)=150`、`first+last=60`、`iter` 求和 `=150`、`Vec<u8>` 字节语义 `=10`、`as_mut_slice` 零拷贝写入 `=9`；184 用例零回归 |
