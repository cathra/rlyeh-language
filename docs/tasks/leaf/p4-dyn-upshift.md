# P4 `&dyn Error` 上转型

> **所属专项**：[专项开发计划](../专项开发计划.md)（P4）
> **来源缺陷**：[`leaf/lang-defects.md`](lang-defects.md) #2（`&dyn Error` protocol 对象构造/上转型失败，源自 Y6）
> **状态**：✅ 已完成（2026-08-28：P4a let 上转型 + P4b vtable 虚调用 + P4c 返回值上转型；Y6 `Option<&dyn Error>` 完整签名留待专项）
> **风险**：中（原「中-高」——已通过 P4a-1~P4c-2 拆分为可独立实现/验证的子步骤；coercion 语义已选自动 a1，避开显式 as 的 `ExprKind::As` 扩展；引用/值上转型路径分离降风险）
> **前置能力**：无（H4 已有 `let d: dyn Protocol = &obj;` 局部变量主路径）

## 目标

支持具体类型 → dyn protocol 对象上转型（`&T as &dyn Protocol` / 自动 coercion），使 Y6 `source() -> Option<&dyn Error>` 可构造非 `None` 值（链式 source 语义可落地）。

## 现状（2026-08-28 探测）

- **症状**：
  - `let d: dyn Error = e;`（e: MyError）→ `expected dyn Error, found MyError`（无自动上转型）
  - `let d: &dyn Error = r;`（r: &MyError）→ `expected &dyn Error, found &MyError`（引用也不能转 dyn）
- **关键实现点**：
  - `coerce_to_dyn`（`check_expr/misc.rs`）：具体类型 → dyn protocol 对象，构造 3 元 vtable 槽（drop/size/align + N 方法槽）
  - 触发点 `check_stmt.rs:66-78`：`let` 注解为 `dyn Protocol`、init 为 `&T` 时调用，同步绑定类型 + 记录 `pending_dyn_concrete` 供去虚拟化
- **根因**：`check_stmt.rs:70` 匹配 `Type::Dyn(protocol_name)`，但 `let d: &dyn Error = r` 注解是 `Type::Ref(Box(Dyn))`（胖指针引用），**不匹配** `Type::Dyn` → 上转型不触发

## 技术方案（拆分，风险逐项化解）

### P4a（核心，中风险）`&T → &dyn Protocol` 引用上转型

扩展 `check_stmt.rs` 匹配 `Type::Ref(Box(Dyn))`，触发 `coerce_to_dyn`；构造胖指针（data + vtable）引用。

**语义选择（先定，影响实现复杂度）**：
- **方案 a1：自动 coercion**（`let d: &dyn Error = r;` 无需 `as`）——与 Rust 一致，需在赋值/let 绑定处做隐式上转型；触发点 `check_stmt.rs:66-78` 已在此（当前仅匹配 `Type::Dyn`，需扩展 `Type::Ref(Box(Dyn))`）
- **方案 a2：显式 `as`**（`let d: &dyn Error = r as &dyn Error;`）——需在 `ExprKind::As` 中新增「具体→dyn」上转型分支，改动更局部但语法更繁琐
- **建议**：先做 a1（与现有 H4 自动 coercion 路径一致，且 Y6 source 链式需要自动），a2 作为后续可选补充

**细化子步骤（方案 a1）**：
- **P4a-1（中风险）触发点扩展**：`check_stmt.rs:70` 匹配从 `Type::Dyn` 扩展为 `Type::Ref(Box(Dyn))`，识别 `&dyn Protocol` 注解；`pending_dyn_concrete` 记录含引用层信息。验收：`let d: &dyn Error = r` 不再报 `expected &dyn Error, found &MyError`。
- **P4a-2（中风险）`coerce_to_dyn` 引用胖指针构造**：构造 data 指向**引用目标**（非引用本身）、vtable 指向具体类型实现；与值上转型（data 指向值拷贝）区分。验收：胖指针布局正确（dyn Error 为 data + vtable 两槽引用）。
- **P4a-3（低风险）`T → dyn Protocol` 值上转型回归**：确认现有 `let d: dyn Protocol = &obj` 值主路径不受影响。验收：H4 既有用例回归全绿。

- **涉及**：`check_expr/misc.rs`（`coerce_to_dyn`）+ `check_stmt.rs:66-78`（触发点）+ `ExprKind::As`（若走 a2）
- **验收**：`let d: &dyn Error = r`（r: &MyError）编译通过，`d` 为两槽胖指针引用

### P4b（中风险）去虚拟化 vtable 槽

`coerce_to_dyn` 的 3 元 vtable（drop/size/align + N 方法槽）在引用上转型路径正确构造（数据指针指向**引用目标**、非被引用栈上对象）。

- **P4b-1（中风险）vtable 布局扩展**：确认 3 元 vtable（drop/size/align）后 N 方法槽的布局与 `pending_dyn_concrete` 去虚拟化逻辑，引用上转型时方法槽绑定到具体类型实现。验收：`d.message()` 经 vtable 定位到 MyError::message。
- **P4b-2（中风险）data 指针语义区分**：值上转型（data 指向栈上值拷贝）vs 引用上转型（data 指向引用目标）——区分两路径的 data 槽填充。验收：`&dyn Error` 的 data 指向 `&MyError` 的目标对象，非引用变量本身。
- **P4b-3（低风险）drop 语义**：引用上转型的 drop 槽（引用不拥有值，drop 应 no-op 或由所有权决定）。验收：守卫/去虚拟化对引用上转型不误 drop 值。

- **涉及**：`coerce_to_dyn` vtable 构造 + `pending_dyn_concrete` 去虚拟化
- **验收**：`d.source()`/`d.message()` 经 vtable 分派到 MyError 实现

### P4c（验证，低风险）Y6 source 链式

`source() -> Option<&dyn Error>` 构造非 `None` 值，链式 `source` 可遍历。

- **P4c-1（低风险）`source()` 返回非 None**：实现 `Error::source` 用上转型构造 `&dyn Error`。验收：`io_error.source()` 非 None。
- **P4c-2（低风险）链式遍历**：`source()` 链（A.source()→B.source()）可遍历至根。验收：Y6 目标签名完整落地，链式用例通过。

- **涉及**：std `io/error.rl`/`error.rl` 的 `Error::source` 目标签名
- **验收**：Y6 目标签名完整落地，source 链式可遍历

## 执行步骤

1. 扩展 `check_stmt.rs` 匹配 `Type::Ref(Dyn)`，触发 `coerce_to_dyn` 上转型（P4a）
2. 支持 `&T → &dyn Protocol` 引用胖指针上转型 + 去虚拟化 vtable 槽（P4b）
3. 验证 Y6 `source() -> Option<&dyn Error>` 可构造非 `None` 值（P4c）

## 验收标准（整体）

- `let d: &dyn Error = r` 编译通过且虚调用正确
- Y6 source 链式实现可落地

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-28 | 由专项开发计划 P4 生成叶子文档（拆分 P4a/b/c） |
| 2026-08-28 | 细化 P4a→P4a-1/2/3（coercion 自动 a1/引用胖指针/值回归）、P4b→P4b-1/2/3（vtable/data 指针/drop）、P4c→P4c-1/2（source 非 None/链式）；整体与 P4a 风险降为中（子步骤均为中/低，语义选择已定） |
| 2026-08-28 | ✅ 完成：`check_stmt.rs` 新增 `&dyn Protocol`（`Ref(Dyn)`）上转型分支（let 绑定 `let d: &dyn Error = r`）；`method.rs` dyn 虚调用扩展到 `Ref(Dyn)` 接收者（vtable 分派 + 去虚拟化）；`fn_sig.rs` 返回值 `&dyn Protocol` 上转型（P4c，Y6 `source() -> &dyn Error` 前提）。验证：`let d: &dyn Error = r; d.message()` → 经 vtable 分派到 MyError::message；`fn source() -> &dyn Error { r }` → 返回上转型成功；H4 值上转型无回归；新增 run-pass `p4_dyn_upshift.rl/.out`；cargo test 全绿。后续：Y6 完整 `Option<&dyn Error>`（胖指针作泛型实参 + 引用生命周期）留待专项 |
