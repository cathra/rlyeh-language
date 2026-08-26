# V2-B：`&str` 子区间视图（trim/trim_start/trim_end）

> **所属任务**：[V2 String 引用视图完整化](../v2-str-view.md)
> **状态**：✅ 已完成（`trim`/`trim_start`/`trim_end` 子区间视图，2026-08-26）
> **依赖**：V2-A
> **权威来源**：`core.rl`（`trim`/`trim_start`/`trim_end`/`as_str_range`）、`rlyeh-typecheck/check_expr.rs` 7770

## 目标

`trim`/`trim_start`/`trim_end` 返回的 `&str` 正确携带 `{data+start, len}` 子区间（零拷贝），替代字符串拷贝。

## 背景

core.rl 的 `trim` 经 `as_str_range(start, end)` 构造 StrFat，但 `v2_probe.rl` 实测未正确剥离空白/体现偏移。

## 实施情况

（已完成，2026-08-26）

### `trim` 子区间视图

- `core.rl` `trim` 正确扫描全空白（空格 32 / 制表 9 / 换行 10 / 回车 13）找 start/end，经 `as_str_range(start, end)` 返回 StrFat 子区间 `{data+start, end-start}`（零拷贝）。
- `as_str_range` typecheck 特判 `PtrAdd` 偏移步长 `FieldScalar::Str` = 1 字节（data 数组 `[u8;0]`）正确。
- 修复 `String::from(&str)` 深拷贝（V2-D）后，`v2_probe.rl` 全部通过（此前非 UTF-8 为陈旧缓存 + 深拷贝 bug）。

### `trim_start`/`trim_end`（新增）

- `core.rl` 新增 `trim_start`（仅剥离前导空白，`as_str_range(start, len)`）、`trim_end`（仅剥离尾随空白，`as_str_range(0, end)`）。
- 单侧剥离正确（xxd 验证 `trim_start("  hello  ")` = "hello  "、`trim_end` = "  hello"）。
- 全空白串 `trim_start`/`trim_end` 返回空视图（len 0）。

### 链式 `&str` 方法（`x.trim().trim()` / `s.trim().to_upper()`）

- 修复：`&str`（StrFat）接收者调 String 方法时，StrFat 是 by_value `{data,len}`，而 String 方法 self 期望 String 对象指针（Ptr）。直接传 StrFat 会污染 LIR `param_types` 传播（同一方法被 String receiver 调用时也误标 StrFat，破坏 `x.trim()`）。
- 方案：`check_method_call` 开头对 `&str` receiver 先经 `make_strfat_to_string` **深拷贝为临时 String**（读 data/len → alloc_bytes + String 3 槽），self 统一为 String 对象，无类型冲突。`&str` 方法返回的 `&str` 指向深拷贝 String 的 data（无 Drop，存活到作用域）。
- `as_str_range` typecheck 特判同时支持 `&String`/`&str`/`str` receiver（`is_string_type` / `is_str_view` / `is_str_value`）。

## 验证

- [x] `v2_probe.rl` 全部通过（剥离空格/制表/换行/回车，子区间偏移正确）。
- [x] 新增 `v2d_str_functions.rl`：`trim_start`/`trim_end` 单侧剥离（xxd 精确字节）、全空白串空视图、`String::from(&str)` 深拷贝。
