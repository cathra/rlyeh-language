# V2-E：String API 对齐 + 兼容保留

> **所属任务**：[V2 String 引用视图完整化](../v2-str-view.md)
> **状态**：✅ 已完成（评估 + 兼容保留，2026-08-26）
> **依赖**：V2-B
> **权威来源**：`std-lib.md` §6.3.3、`core.rl`（`chars`/`lines`/`trim`）

## 目标

`chars`/`lines`/`trim` 目标签名对齐，旧 API 保留兼容。

## 背景

目标 `chars(&self) -> Chars`（返回 `Option<char>`），当前用 `chars_iter -> Chars`（码点 i64）；`lines` 同理；`trim -> &str` 替代拷贝。

## 实施情况

（已完成，2026-08-26）

### API 对齐决策

- **`chars`/`lines`（返回 Vec）保留为兼容 API**：`string_api.rl` 等现有调用（`s.chars()`/`t.lines()`）依赖其返回 `Vec<i64>`/`Vec<String>`，升级签名会破坏现有代码。目标签名（`chars(&self) -> Chars` 返回迭代器）**暂不升级**，作为兼容决策记录。
- **迭代器版并行存在**：`chars_iter`/`lines_iter` 返回 `Chars`/`Lines` 迭代器（V2-A 已实现），`next()` 经 match 解包返回字节/行。已验证正常（`chars_iter.next()` → 97/98/99/-1）。
- **`trim`/`trim_start`/`trim_end` 返回 `&str` 视图**：V2-B 完成（子区间 StrFat，零拷贝）。
- **String 方法与 `&str` 视图协同**：`split("\n")`/`contains`/`substring` 保持原语义；`&str`（StrFat）可传参/返回/`String::from(&str)` 深拷贝（V2-D）。

### 待办（目标 API，不在本任务）

- `chars` 目标签名（返回 `Chars`，元素 `Option<char>`）需 `char` 类型支持后升级（规划）。

## 验证

- [x] 旧 `chars()->Vec`/`lines()->Vec` 调用继续工作（`string_api.rl` 通过）。
- [x] `chars_iter`/`lines_iter` 迭代器正常工作（match 解包字节/行）。
- [x] `trim`/`trim_start`/`trim_end` `&str` 视图（`v2_probe.rl`/`v2d_str_functions.rl` 通过）。
- [x] 文档 std-lib.md §6.3.3 同步更新。
