# V2-D：`&str` 参数/返回值完整支持

> **所属任务**：[V2 String 引用视图完整化](../v2-str-view.md)
> **状态**：✅ 已完成（`&str` 函数参数 ✅；返回值 borrowck ✅；`String::from(&str)` 深拷贝 ✅；`&String`↔`&str` 隐式转换 ✅）
> **依赖**：V2-A
> **权威来源**：`rlyeh-lir/lower.rs`（`param_types`）、`rlyeh-borrowck/checker.rs`（`check_dangling_strfat_block`）、`guide.md` §10.2

## 目标

`&str` 在函数参数（`&String`/`&str` 均可传入）、返回值、`String::from(&str)` 深拷贝的完整支持。

## 背景

guide.md 990 声称已支持参数/返回/索引/深拷贝，但 StrFat 双槽化后需重新验证借用与跨函数传递。

## 实施情况

### `&str` 函数参数（已完成，2026-08-26）

**根因**（两层）：
- ① 用户函数参数 LIR 默认 `i64`（`str_len` 内联后无 `Call`，跨函数参数类型无从推断）。
- ② 即使 `String::len({i8*,i64} %self)` 参数签名正确，函数体内 `self.addr` 仍按 `ty`（locals 推断为 i64）声明，`self.len` 走通用 FieldGet（读 i64 槽 +8）读到错误偏移。

**修复（rlyeh-lir lower.rs）**：
- ① 新增全程序 `param_types` 表（函数名 → 各参数 LirType），在 infer 循环遍历各函数 `Call`，实参为 StrFat（`&str`）时传播到被调函数对应参数。
- ② `lower_function` 生成参数时优先取 `param_types`（实参为 StrFat → 参数 StrFat），并**同步写回 `ty`（locals）**——确保函数体内参数局部变量 codegen 类型与签名一致（`self.addr` 正确声明为 `{i8*,i64}`，`self.len` 走 StrFat FieldGet）。

**验证**：`v2a_str_semantics.rl` 第 3 行 `str_len(v)` 由 0 变 5；全量 137 通过 0 失败（仅 2 async 跳过）。

## 待办（返回值/深拷贝）——已完成

### 返回值 borrowck（已完成，2026-08-26）

- `rlyeh-borrowck/checker.rs` 新增 `check_dangling_strfat_block`：识别 `as_str`/`as_str_range` 构造的 StrFat 返回块（`Alloc{is_strfat}` + FieldSet data/len），判定 data 来源：
  - **参数来源**（`String`/`&str` 参数）→ 允许返回（`fn take_str(s: String) -> &str { s.as_str() }` 通过）。
  - **局部来源**（函数内 `let s = String::from(...)`）→ 报 `DanglingReference`。
- 新增 `tests/compile-fail/v2d-return-local-str.rl` 覆盖。

### `String::from(&str)` 深拷贝（已完成，2026-08-26）

- 修复 `check_string_from` 的 `&str` 分支（`check_expr.rs` 5099+）：原实现 `base` 同时作数据缓冲（`copy_bytes`）与 String 对象头（FieldSet 3 槽），前 24 字节被覆盖破坏数据。
- 改为**两个独立分配**：独立 `alloc_bytes(len+1)` 数据缓冲 + 独立 `Alloc{slots:3}` String 对象（对齐字面量路径）。

### 隐式转换（已完成）

- `&String` → `&str`（`a.as_str()` 传 `&str` 参数）、`&str` 传参均正确（LIR `param_types` 传播）。

## 验证

- [x] `fn f(s: &str) -> i64 { s.len() }` 传入 `&String`/`&str` 均正确（2026-08-26，`str_len` 返回 5）。
- [x] `fn g(s: &str) -> &str { s }`（参数来源引用返回）通过 borrowck。
- [x] `fn take_str(s: String) -> &str { s.as_str() }` 返回参数 String 的 `&str` 通过并运行正确（"world"）。
- [x] `String::from(&str)` 深拷贝为独立 String（`v2d_str_functions.rl`：copy=="world"，len 5，== src true）。
- [x] 返回局部 String 的 `&str` 报 `DanglingReference`（`v2d-return-local-str.rl`）。
