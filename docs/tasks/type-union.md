# 受限制的类型联合（规划）

> **状态**：✅ 已完成（2026-08-31；U1–U5 全部落地，全量 194 用例零回归）
> **定位**：对现有 **enum 标签联合体系**的增强（**非新运行时类型**）
> **核心约束（"受限制"）**：联合成员必须**两两互不相交（disjoint）**，保证 tag 无歧义、收窄安全

**叶子任务文档**：[`union-u1-type-layer.md`](leaf/union-u1-type-layer.md) · [`union-u2-desugar-match.md`](leaf/union-u2-desugar-match.md) · [`union-u3-scalar-enum-plan.md`](leaf/union-u3-scalar-enum-plan.md) · [`union-u4-field-union.md`](leaf/union-u4-field-union.md) · [`union-u5-tests-docs.md`](leaf/union-u5-tests-docs.md)

## 1. 目标

在现有具名 tagged `enum` 之上，提供三类增强，统一覆盖"一个值可能是多种类型之一"的表达：

1. **匿名类型联合 `T | U | ...`**——类型级联合，作为 `enum` 的匿名语法糖。
2. **字段级联合类型**——`struct` 字段类型可写联合。
3. **受限标量枚举（C-like enum / 值域枚举）**——变体为互不相交标量/单元时的紧凑表示与放宽使用。

## 2. 设计原则

- **复用现有 enum 机制**：匿名联合 desugar 为编译器生成的匿名 `enum`（每个成员 → 一个携带该类型的变体 + tag 槽）。**不引入新的运行时表示**，仅新增类型层 + desugar。
- **与具名 enum 的关系**：匿名联合 = 轻量匿名版；具名 `enum` 继续用于需要命名/方法/显式判别值的场景。
- **收窄（narrowing）**：对联合值用 `match` 按成员类型分支（复用现有 `check_match` 的 tag 比较 + 字段提取）；可选 `x is T` 类型守卫（MVP 可仅支持 `match`）。

## 3. 子特性设计

### 3.1 匿名类型联合 `T | U`

- **语法**：类型上下文允许 `|` 链：
  ```rlyeh
  let x: i64 | String = ...;
  fn f(p: i64 | f64) {}
  let r: Option<i64> | IoError = ...;
  ```
- **表示**：`Type::Union(Vec<Type>)`（新增）。`resolve_ast_type` 时检查成员互不相交（见 §4）。
- **构造**：联合值由任一成员类型的值直接赋值得到（成员 ⊆ 联合，协变）。
- **使用**：必须 `match` 收窄后才能按具体类型操作；未收窄禁止直接方法调用/运算（`typecheck` 报错）。
- **方法调用**：仅当某方法对所有成员都存在且签名一致时可经联合调用（可选，MVP 不做）。

### 3.2 字段级联合类型

- `struct` 字段类型可写联合：
  ```rlyeh
  struct S { id: i64 | String }
  ```
- 语义 = 该字段为匿名联合（等价于 3.1，字段级应用）。
- 字段读写遵循 3.1 的收窄规则。

### 3.3 受限标量枚举（C-like enum / 值域枚举）

- **定义**：枚举变体**不携带负载**或仅携带互不相交的标量负载，且变体间判别式互不重叠。
- **表示优化**：存为单个标量（tag 槽即值域，无需 payload 槽）；`EnumDef::slot_count` 退化为 1。类比 Rust fieldless enum = 整数。
- **放宽使用**：可在需要标量的上下文使用（作为数组索引、位运算、与整数比较）；可选显式判别式 `enum E { A = 1, B = 2 }`。
- **与 3.1 区别**：3.3 是"枚举本身的受限模式"（值域），3.1 是"类型的匿名联合"。两者互补。

## 4. 互不相交（disjoint）约束（受限制的核心）

`typecheck` 阶段对 `Type::Union(members)` 校验：

- 不允许重复成员（`i64 | i64` 非法 → 合并为单成员或报 `UnionMembersNotDisjoint`）。
- 不允许可重叠成员：如 `&T | &mut T`（引用可变性重叠）、`i64 | isize`（平台相关重叠）报 `UnionMembersNotDisjoint`。
- 不同具名类型 / 枚举成员视为不相交（MVP 不引入子类型，故均不相交）。
- 该检查在 `resolve_ast_type` 构建 `Union` 时执行，错误类型 `UnionMembersNotDisjoint`。

## 5. codegen 设计

- **匿名联合 → 生成匿名 `EnumDef`**（`EnumDef` 动态注入类型环境，tag 槽 + 各成员最大尺寸 payload 槽），与现有 `enum` codegen **完全复用**，无新增 codegen 通道。
- **收窄 `match u { T => .., U => .. }`**：复用现有 match 的 tag 比较 + `FieldGet` 提取；因成员即类型，分支模式可简化为**类型臂（type arm）**。
- **受限标量枚举**：单标量存储，`match` 退化为整型比较；可与整数互操作。
- 复用资产：`EnumDef` / `VariantDef` / `check_match` / `field_scalar_of`、现有 enum codegen。

## 6. 与现有系统关系

- **复用**：`EnumDef`/`VariantDef`/`check_match`/`field_scalar_of`、现有 enum codegen。
- **不影响**：具名 `enum`、`Option`/`Result`（仍是具名 enum）；联合是它们的匿名补充。
- **与切片正交**（见 `slice-type-system.md`）：可组合 `&[i64 | String]`，联合嵌套于切片元素已支持（见 §9 ②，2026-09-21）。

## 7. 验收标准

- [x] `let x: i64 | String` 声明 + 由 `i64`/`String` 赋值 + `match` 收窄编译通过。
- [x] 字段级联合 `struct S { id: i64 | String }` 可用 + 收窄。
- [x] 互不相交校验：重叠成员报 `UnionMembersNotDisjoint`。
- [x] 受限标量枚举：单标量存储 + 可作数组索引/位运算。
- [x] `tests/run-pass` 联合用例全绿，与现有 `enum`/`Option`/`Result` 行为无回归。

## 8. 任务拆分（分阶段）

- **U1 类型层** **✅ 已完成（2026-08-30；`Type::Union` + parser `|` 链 + `resolve_ast_type` 构建 `Union` 并 `check_union_disjoint` + `Display`/`compatible_with`/`field_scalar_of`，见 CHANGELOG U1/U2）**。
  - `Type::Union(Vec<Type>)`；`parser/src/ty.rs` 支持 `|` 链（primary 类型后遇 `|` 合并为 `Union`）。
  - `resolve_ast_type` 构建 `Union` + disjoint 校验；`Display` / `compatible_with`（收窄前不允许运算）/ `field_scalar_of`（按最大成员尺寸）。
- **U2 desugar + match 收窄** **✅ 已完成（2026-08-30；联合 → 匿名 `EnumDef` 注入 + `make_union_ctor` 向上转换 + `check_match` 类型臂收窄，见 CHANGELOG U1/U2；`union_basics` 用例）**：联合 → 匿名 `EnumDef` 注入；`check_match` 支持类型臂（type arm）收窄；`x is T` 守卫（可选）。
- **U3 受限标量枚举**：enum 变体全标量/单元时 `slot_count = 1` + 标量存储 + 放宽到整数上下文。**✅ 已完成（2026-08-30；`Type::ScalarEnum` + 4 联动 + 缓存键 bump + 整数上下文放宽 + 比较对称；`enum_scalar_context` / `enum_scalar_storage` 用例 + `compile-fail/enum_scalar_no_int_to_enum`；191 用例全绿）**。
- **U4 字段级联合**：`struct` 字段类型解析支持 `Union` + 读写收窄。**✅ 已完成（2026-08-31；复用 U1/U2 匿名联合 machinery——`struct S { id: i64 | String }` 字面量构造与字段赋值 desugar 为匿名 enum 构造、字段读取须 `match` 收窄、disjoint 校验与值级一致；`struct_field_union` + 两个 compile-fail 用例；194 用例全绿）**。
- **U5 测试与文档** **✅ 已完成（2026-08-31；`union_basics`/`enum_discriminant`/`enum_scalar_*`/`struct_field_union`(+2 compile-fail) 用例齐备 + `grammar.md` §2.4 / `semantics.md` §8.4–§8.5 / `CODEBUDDY.md` 章节齐全，见 leaf `union-u5-tests-docs.md`）**：`tests/run-pass` 联合用例；`grammar.md`/`semantics.md` 补联合章节。

## 9. 风险与开放问题

- `|` 在类型上下文的解析优先级（与闭包 `|x|`、位或区分——类型上下文无歧义，但需明确 `fn(T | U) -> R` 的括号规则）。
- 联合的方法分发（MVP 不做，标注为开放问题）。**✅ 已实现（2026-09-21）**：`check_method_call` 在接收者为 `Type::Union` 时 desugar 为按成员类型臂分发的 `match`（零新增 IR / codegen 通道），方法须对所有成员存在且签名一致，否则报 `UnionMethodNotCommon`（`TC026a`）；验收 `tests/run-pass/union_method_dispatch.rl` + `tests/compile-fail/union_method_missing_member.rl`，全量套件零回归。
- 联合嵌套于切片元素（MVP 限制）。**✅ 已实现（2026-09-21）**：`&[i64 | String]` 等「切片元素为联合」已支持——数组字面量在期望元素类型为 union 时逐元素 `make_union_ctor` 包成真正的 `[union; N]`；切片索引 / match 类型臂收窄 / 再切片（`xs[1..<3]`）/ `.len()` 均工作（验收 `tests/run-pass/union_slice_element.rl`）。核心设计约束：**切片为布局敏感类型，unsize coercion `&[T; N] → &[U]` 仅当 `T == U`（元素类型完全相同）方可降级**，故 `&[i64; 3]` 不可 coerce 成 `&[i64 | String]`（步长不同，按值 reinterpret 属未定义行为）；违反时类型检查拒绝（`TC018`，见 `tests/compile-fail/union_slice_coerce_mismatch.rl`）。该约束同时收紧于 `types.rs` 的 `compatible_with` S2 规则与 `call.rs` / `check_stmt.rs` 的 unsize coercion 站点。
  - **切片迭代 pre-existing bug 已修复（2026-09-21）**：此前「指针元素切片」（`&[String]`、`&[i64 | String]` 等）的 `for x in xs.iter()` 错位读取——`Slice::iter()` 被 desugar 成 `IterRef<T>`，其 `next() -> Option<&T>` 经 `&self.data[0]` 返回**元素槽地址 E**（指向存「对象指针 O」的 8 字节槽），而方法接收者需要直接的对象指针 O，二者不一致导致 `x.len()` 读 `*(E+8)` 垃圾（与联合无关，属通用语言 bug）。修复：`method/builtin.rs` 的 `iter()` desugar 对 `field_scalar_of(elem_ty) == Ptr` 的元素改走**值迭代器 `Iter<T>`**（`next() -> Option<T>` 经 `self.data[0]` 索引直接读出 O，与 `cs[0]` 索引语义一致），故 `x` 即为可用作方法接收者的对象指针；`Iter::next` / `IterMut::next` 同步由 `*self.data` 改为 `self.data[0]`（对值类型语义等价，对指针类型修正为读出 O）。非指针元素（i64 等）保持 `IterRef<T>` 以保留原地写回语义。验收 `tests/run-pass/union_slice_iterate.{rl,out}`（`&[i64|String]` for 迭代 + match，输出 `32`）+ `tests/run-pass/string_slice_iterate.{rl,out}`（`&[String]` for 迭代 `.len()`，输出 `1/1/2/3`）；全量 `rlyeh test tests` **350/350 通过**。**残余限制**：`iter_ref()`（显式 `IterRef<T>`、`next() -> Option<&T>`）对指针元素切片仍返回槽地址 E，方法分发 / 原地写回不可用（指针元素切片槽仅 8 字节存 O，本身无法承载完整对象写回）——改用 `.iter()` 值迭代即可规避；主路径（索引 + match 收窄）不受影响。
- 联合与泛型/`dyn Protocol` 的组合边界（MVP 限制：联合成员不含 `dyn Protocol`）。
