# V2-C：`&str` 打印链路修复（含 by_value StrFat 运行时）

> **所属任务**：[V2 String 引用视图完整化](../v2-str-view.md)
> **状态**：✅ 已完成（方案 A，2026-08-26）
> **依赖**：V2-A
> **权威来源**：`rlyeh-hir/lib.rs`、`rlyeh-mir/lib.rs`、`rlyeh-lir/lower.rs`、`rlyeh-codegen/llvm.rs`

## 目标

`println(&str)` 走 StrFat `%.*s` 长度限定打印，输出子区间内容而非瘦指针地址。

## 背景

typecheck 打印路径（check_expr.rs 2625）对 `Ref(Str)` 剥层为 `Type::Str`，丢失 len；LIR 对 println 参数 StrFat 值推断为 `Ptr`。V2-A 审计确认完整缺口。

## 根因定位（逐层 IR 调试）

**StrFat 的 `.addr` 值槽与 by_value `.obj` 指针机制的冲突**：
- 1175：所有局部变量 `.addr = alloca {llvm_type}`，对 StrFat 为 `alloca {i8*,i64}`（**值槽**）。
- 1426-1431（by_value `Alloc` 发射）：`store i8* (bitcast .obj), i8** %target.addr`——把 `.addr`（`{i8*,i64}*`）**bitcast 成 `i8**`** 写入 `.obj` 地址，**污染 `.addr` 指向的 `{i8*,i64}` 对象的 data 槽（=.obj 地址）**。
- 后续 FieldSet 经 `operand_value(base, Ptr)` 读 `.addr` 前 8 字节作为"对象指针"解引用，把 data 写到错误地址。
- 最终 `v = {data: 垃圾, len: 正确}`，打印 `%.*s` 读 data 槽得到垃圾。

## 实施情况（方案 A）

**核心改动**：
- **HIR/MIR 增加 `is_strfat` 标记**：`HirExpr::Alloc`/`MirStmt::Alloc` 增 `is_strfat: bool`（`as_str`/`as_str_range` 构造时置 `true`），`inline` pass 透传。LIR 据此精确推断 StrFat——**不误伤**普通双槽 by_value 结构体（`Point{x,y}`，`addr_of_field_index`）。
- **codegen 方案 A**：by_value_locals 排除 StrFat（不走 `.obj`）；`Alloc` 对 StrFat 清零 `.addr` 值槽（`{i8*,i64}`）；`FieldGet`/`FieldSet` 对 StrFat 直接 GEP `.addr` 值槽（不读 `.addr` 值当指针）。避免 by_value `.obj` 污染 data 槽（根因）。
- **typecheck**：`println(&str)` 参数**保留 StrFat**（不剥层为 `Str`）。
- **修复换行 bug**：StrFat 打印的 fmt 换行 `"\\n"`（字面量）改为 `"\n"`（真实换行，llvm.rs 2722）——否则 `%.*s` 打印输出字面量 `\n` 而非换行，导致 `.out` 不匹配。
- **LIR resolve 更新 `ty`**：`resolve_call_target_types` 直接更新 `ty`（非克隆 `resolved`），使调用点 `r = as_str_range(...)` 的 `ty[r] = StrFat`（否则打印走 Ptr 分支输出指针地址）。

## 验证

- `v2c_print.{rl,out}`（as_str/as_str_range/trim 打印）通过。
- `v2_probe.{rl,out}`（trim 剥离）通过。
- `addr_of_field_index` 无回归。
- **遗留（已转 V2-D）**：`&str` 函数参数（`str_len(x:&str)` 返回 0），见 [V2-D](./v2-d-params.md)。
