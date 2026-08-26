# V2-A：`&str` 语义统一为 StrFat 双槽

> **所属任务**：[V2 String 引用视图完整化](../v2-str-view.md)
> **状态**：✅ 已完成（审计 + 文档澄清，2026-08-26）
> **依赖**：无
> **权威来源**：`guide.md` §10.2、`std-lib.md` §6.3.3、`rlyeh-typecheck/types.rs` 406

## 目标

澄清并落地 `&str` 为 StrFat 双槽胖指针 `{data, len}`，取代"瘦指针（指向整个 String 对象）"的既有语义。

## 背景

当前 `&str`（`Type::Ref(Str)`）在 typecheck 中 `FieldScalar` 为 `StrFat`（types.rs 406），但 guide.md 与多处打印路径仍按瘦指针处理，导致语义分裂。

## 实施情况

### 审计结论（2026-08-26）

- ✅ **typecheck 构造层已统一为 StrFat**：`field_scalar_of(Ref(Str)) = StrFat`；`as_str`（check_expr.rs 7686）与 `as_str_range`（7737）均构造 StrFat 双槽 `{data, len}`。`v2a_str_semantics.rl` 验证 `&str` 的 len/索引/参数/子区间均正确。
- ⚠️ **运行时缺口在打印链路（移交 V2-C）**：
  - **LIR 推断**：`MirStmt::Alloc{by_value, slots:2}` 被推断为 `Ptr`（lower.rs 175），非 StrFat；当前靠函数名硬编码特判 `as_str_range`/`String::trim`（lower.rs 66），`as_str` 漏覆盖。
  - **codegen by_value StrFat**：as_str 构造的 StrFat by-value 值在逃逸分析中因"作实参传出"（`Call`）被剔除 by_value 改走 calloc，且打印分支 `%{arg}.addr` 读取 data 槽错乱 → 实测 `println(&str)` 读到垃圾。**尝试直接修复引入打印回归（`pX\n`/非 UTF-8），已回退**——完整修复需 codegen by_value StrFat 的 `.addr`/`.obj` 布局协同，属 V2-C。
- ✅ **已回退所有运行时改动**，工作区恢复到稳定状态。

### 本阶段改动

- 文档（guide.md §10.2、std-lib.md §6.3.3）：澄清 `&str` = StrFat 双槽 `{data, len}`；瘦指针仅适用于字符串字面量 `str` 值。
- 确认 `field_scalar_of(Type::Str)`（瘦指针）与 `Ref(Str)`（StrFat）的区分在所有构造点一致。

### 移交 V2-C 的发现

① LIR 按类型（而非函数名）识别 `&str` 返回类型；② codegen by_value StrFat 的 `.addr` 读取 / 逃逸分析豁免打印内建。

## 验证

- [x] 审计确认 typecheck 构造层 `&str` 已是 StrFat（as_str/as_str_range）。
- [x] 文档澄清 `&str` = StrFat 双槽语义。
- [x] `v2a_str_semantics.rl` 通过（len/索引/参数/子区间）。
- [ ] 打印运行时缺口移交 V2-C（不在本任务修复）。
