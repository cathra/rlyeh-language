# U3 受限标量枚举

> **所属**：受限制的类型联合（规划 [`type-union.md`](../type-union.md)）
> **状态**：✅ 已完成（2026-08-30；**显式判别式** + **单标量存储** + **整数上下文**核心项全部落地；`rlyeh test` 190 用例全通过；专项计划见 [`union-u3-scalar-enum-plan.md`](./union-u3-scalar-enum-plan.md)）
> **阶段**：U3（受限标量枚举）

## 目标

enum 变体全为不相交标量/单元时紧凑表示（单标量存储）+ 放宽到整数上下文。

## 技术细节

- 判定：所有变体不携带负载或仅携带互不相交标量负载 → `EnumDef::slot_count = 1`，存为单标量（tag 即值域）。
- 放宽使用：可作数组索引、位运算、与整数比较；可选显式判别式 `enum E { A = 1, B = 2 }`。
- 与 U1 匿名联合互补（U1 是类型匿名联合，U3 是枚举受限模式）。

## 验收

- [x] **显式判别式**（`enum E { A = 1, B = 2 }`）正确：判别值在收集阶段写入
      `VariantDef::tag`，构造与 `match` 的 tag 比较均复用之（IR 实测
      `icmp eq i64 %x, 404` 而非序号），判别值与序号不一致时仍正确。
- [x] **全标量/单元枚举单标量存储**：`Type::ScalarEnum(String)` 表示（值即 tag，
      `field_scalar_of` 返回 `Int`）；`check_variant_construct` 直接产出 `IntLiteral(tag)`，
      不再 `Alloc` 对象指针。新增 `tests/run-pass/enum_scalar_context.rl` 验证。
- [x] **可作数组索引/位运算/整数比较**：标量枚举可作数组索引（`check_index` 放宽）、
      位运算（`check_binary` 放宽，结果按 `i64`）、与整数比较及单向赋值给整数
      （`compatible_with` 单向：期望整型、实参标量枚举为 true；反向禁止，新增
      `tests/compile-fail/enum_scalar_no_int_to_enum.rl`）。

### 第四轮成功（2026-08-30）——根因与修复

前三轮失败的根因**并非**「收集没跑到」或「use 别名时机」（这两层经 `collect_enums_first`
/ `register_uses_first` 已解决且行为不变），而是 **ScalarEnum 分支只挂在 `resolve_named_type`
的「单条」枚举返回路径上**（`crates/rlyeh-typecheck/src/context.rs` 裸名分支），而另一条
「模块路径 / use 别名」分支（`resolve_full_name` 回退路径）仍产出 `Type::Named` —— 同一枚举
的两种表示并存，`field_scalar_of` 布局判定（无 `TypeContext`、只看类型本身）自相矛盾。

**修复**：`resolve_named_type` 的**两条**枚举返回路径统一在 `is_scalar_enum` 判定后产出
`Type::ScalarEnum`，使主类型检查 ctx 与 `collect_fn_signatures` 接口路径（两者共用同一
`resolve_named_type`）产出的类型表示始终一致。配合 4 联动（构造 / 存储类别 / 收窄 / 放宽）
及 `INTERFACE_VERSION` 由 1 递增为 2（使表示变更前的旧增量缓存失效），一次通过。

> 此前三轮尝试的回退代码（含 `register_uses_first`/`collect_enums_first` 预扫描）已确认非
> 必需——当前实现不依赖预扫描，因为 `resolve_named_type` 在收集阶段 enum 已注册后即可正确
> 判定 `is_scalar_enum`，两条路径天然一致。

## 现状调研（2026-08-30）

- ✅ fieldless enum 的 `match` **已正常工作**（实测 `enum Color { Red, Green, Blue }`
  匹配 `Color::Green` 输出 `1`）。
- ❌ **枚举尚不能用于整数上下文**：`let i: i64 = c;` 报
  `expected 'i64', found 'Color'`。
- 现状表示：fieldless enum 的 `slot_count` 已自然为 `1`（`1 + max(变体字段数=0)`），
  但构造仍是 `Alloc(1) + FieldSet(0, tag)`——**值是对象指针**，tag 存在所指槽中，
  故不能直接当标量用。

## 实现路径（4 处联动，须同批改动）

1. **构造**（`check_variant_construct`）：标量枚举直接产出 `HirExpr::IntLiteral(tag)`，
   不再 `Alloc`——值即 tag 本身。
2. **存储类别**：`field_scalar_of` 对标量枚举返回 `FieldScalar::Int`（当前 `Named(..)`
   一律返回 `Ptr`）。
3. **收窄**（`check_pattern` 的 `AstPattern::Enum` 分支）：标量枚举的 `cond` 改为
   `scrutinee == tag` 直接整数比较（当前为 `FieldGet(scrutinee, 0) == tag`）。
4. **放宽**（`compatible_with`）：标量枚举 → 整数单向兼容（读取 tag）。
   建议**单向**：禁止整数 → 枚举，避免构造出无对应判别式的非法值。

### 关键障碍

`field_scalar_of(ty: &Type)` **不持有 `TypeContext`**，无法查 `EnumDef` 判定
「该具名类型是否为标量枚举」。可选解法：

- (a) 为该函数增加 `ctx` 参数（调用点众多，改动面大）；
- (b) 在 enum 收集期把「标量枚举」信息编码进类型层（如新增 `Type::ScalarEnum(String)`
      或给 `Named` 附带标记），使 `field_scalar_of` 无需 ctx 即可判定；
- (c) 在收集期为标量枚举单独维护一张名字集合（全局单例 / `TypeContext` 外的静态表），
      供无 ctx 的纯函数查询。

推荐 (b)：信息随类型走，语义最清晰；代价是 `Type` 新增变体需补各处穷尽匹配
（同 U1 加 `Type::Union` 时补了 parser / doc / fmt 三处）。

### 首次尝试与回退（2026-08-30）

按解法 (b) 实装了「类型层编码」方案：新增 `Type::ScalarEnum(String)` → `field_scalar_of`
返回 `Int`；构造直接产出 `IntLiteral(tag)`；收窄走直接整数比较；`compatible_with` 与整数
互操作。**编译零错误**，但运行后 **188 用例中 184 个失败**，已全部回退（仅保留
`TypeContext::is_scalar_enum` 判定待复用）。

**根因：收集顺序依赖** —— 同一 enum 出现了两种类型表示，且槽布局不一致：

- `IoError` 结构体的 `kind: IoErrorKind` 字段在 **`IoErrorKind` 枚举注册之前**解析，
  固化为 `Type::Named`（`field_scalar_of` → `Ptr`）；
- 调用点实参在检查阶段解析，此时 enum 已注册 → `Type::ScalarEnum`（→ `Int`）。

二者 `Display` 相同（错误信息因此形如 `expects IoErrorKind, found IoErrorKind`，极具
迷惑性），但 `Ptr` 与 `Int` 布局不同 → 传参与字段读写全面错乱。因 IO 贯穿几乎所有
用例，影响面达 184/188。

**修复前提（须先做其一，再重做本叶子）**：

1. **两遍收集**：先扫描注册全部 `enum`，再收集 `struct` / `impl` / `fn`，确保解析字段
   类型时 enum 已注册；或
2. **类型规范化 pass**：收集完成后统一遍历 `StructDef` 字段、`FnSignature` 参数与返回
   类型，把已注册标量枚举的 `Named(n)` 替换为 `ScalarEnum(n)`。

核心不变式：**同一枚举的类型表示必须全局唯一**，否则 `field_scalar_of` 的布局判定
（该函数无 ctx、只能看类型本身）会自相矛盾。

### 第二次尝试与回退（2026-08-30）

先按上述「两遍收集」方案做了 enum 预扫描（`collect_enums_first`：在
`collect_declarations` 之前注册全部**全单元变体** enum；只预注册无负载者，因其无需
`resolve_ast_type`，可安全提前）。**预扫描单独跑 188 全通过**（行为不变，符合预期）。
但重新应用 U3 后**仍 184 失败**，且是同一个错误 —— 说明首轮的归因不准确。

**更精确的根因：问题不在「enum 是否注册」，而在「裸名解析依赖 use 别名」**。

- 收集阶段（`collect_impl` 解析 `IoError::new(kind: IoErrorKind, ..)`）时，
  `use_aliases` **尚未注册**（`register_use` 在 `collect_declarations` 内），故裸名
  `IoErrorKind` 无法映射为完整名 `io::error::IoErrorKind`；
  `is_scalar_enum("IoErrorKind")` 判定为 false → 形参类型固化为 `Named`。
- 调用点在检查阶段解析时 use 已注册 → 得到 `ScalarEnum("io::error::IoErrorKind")`。

二者 `Display` 仍是同一个字符串，报错依旧是 `expects IoErrorKind, found IoErrorKind`。

**下次的正确方向（三选一）**：

1. **use 别名先行**：在任何类型解析之前先注册全部 `UseDecl`（递归 mod），再预扫描 enum。
   最贴合现有架构，推荐优先尝试。
2. **`is_scalar_enum` 支持裸名**：按完整名的**后缀**匹配（`/::/` 分段末段相等即命中），
   绕开 use 别名的时机问题；代价是同名枚举可能误判。
3. **延迟签名解析**：把 fn / impl 方法的签名类型解析也挪到第二遍（与 struct 字段
   `resolve_all_struct_fields` 同款），此时 use 与 enum 均已注册。改动最大但最彻底。

两轮尝试均已回退（含移除预扫描），当前 **188 用例全通过、代码干净**。
`TypeContext::is_scalar_enum` 判定保留待用（带 `#[allow(dead_code)]`）。

### 第三次尝试与回退（2026-08-30）

按修法 1 实装：`register_uses_first`（在任何类型解析前注册全部 `use` 别名）+ `collect_enums_first`（预注册全单元变体 enum），均在 `typecheck_with_region_hints` 的 `collect_declarations` 之前。预扫描单独跑 188 全通过（行为不变）。

重新应用 U3 后**仍 184 失败**，且错误进化为：两端都是完整名 `io::error::IoErrorKind`（Display 相同），仅变体不同——形参 `Named`、实参 `ScalarEnum`。这证明 use 别名时机已不是主因。

**加调试定位于第三个独立障碍**：`resolve_ast_type` 全程**未被**以 `IoErrorKind` 调用（无日志），但 `is_scalar_enum("io::error::IoErrorKind")` 被调用且返回 true（来自 `check_variant_construct` 的实参变体构造）。即——

> **`IoError::new` 的「形参类型」来自一条不经过 `resolve_ast_type` 的解析/缓存路径**（很可能是 std 函数签名的独立解析，或增量模块接口 `extract_interface`/`collect_fn_signatures` 的缓存），它产出旧 `Type::Named`；而「实参」（变体构造）走 `check_variant_construct` 实时解析为 `ScalarEnum`。二者由此不一致。

即便 `--force` 绕过用户缓存仍失败，说明 std 签名存在**独立于用户 `--cache-dir` 的解析/缓存路径**（或签名在收集阶段经非 `resolve_ast_type` 的路径落库）。

**结论**：U3 核心项不是「两三个局部修复」能收口的特性，而是**贯穿类型层、收集层、签名落库/缓存层的表示一致性问题**。当前已暴露三层障碍：

1. 收集顺序依赖（已用 `collect_enums_first` 解决，行为不变）；
2. use 别名时机（已用 `register_uses_first` 解决，行为不变）；
3. **std 函数签名的解析/缓存路径与 `resolve_ast_type` 不一致**——这是真正的硬骨头，须先摸清签名落库的全部分支（collect_impl / collect_fn_signatures / 增量 interface 缓存）并统一为同一解析入口，否则任何单点改动都会被另一条路径的 `Named` 抵消。

三轮尝试均已回退（含移除 `register_uses_first` / `collect_enums_first` 与全部调试），当前 **188 用例全通过、代码干净**。`is_scalar_enum` 判定保留待用。

### 风险

改变 fieldless enum 的表示（对象指针 → 标量值）会影响**所有**已存在的 fieldless enum
（含 std 中的定义，如各类错误 Kind / 状态枚举）。须先盘点 `core.rl` 与 tests 中的
fieldless enum，并全量回归（当前基线 186 用例）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-30 | 由联合规划细化为叶子 |
| 2026-08-30 | 现状调研：fieldless enum 的 match 已可用，但枚举不能用于整数上下文；记录 4 处联动实现路径、「`field_scalar_of` 无 ctx」关键障碍与三种解法、以及先盘点 std fieldless enum 的风险提示 |
| 2026-08-30 | 盘点 std fieldless enum：`IoErrorKind` / `Interest` / `OpenMode` / `Shutdown` 共 4 个（贯穿 IO 与网络，如 `File::open_with(path, OpenMode::Read)`），确认表示变更的影响面 |
| 2026-08-30 | 实现：**显式判别式** `Variant = 42`——`AstEnumVariant::discriminant`（rlyeh-ast）+ parser `item.rs` 解析 `= <整数字面量>` + 收集阶段 `collect.rs` 写入 `VariantDef::tag`；构造与 `match` 复用 tag 故自动生效。新增 `tests/run-pass/enum_discriminant.{rl,out}`；IR 验证 `icmp eq i64 %x, 404`；`rlyeh test` 187 用例零回归 |
| 2026-08-30 | **U3 核心项落地（单标量存储 + 整数上下文）**：新增 `Type::ScalarEnum(String)` 表示；`resolve_named_type` 两条枚举返回路径统一经 `is_scalar_enum` 判定产出 `ScalarEnum`（修复前三轮「单路径分支」导致同枚举两种表示的根因）；4 联动——`check_variant_construct` 直接产出 `IntLiteral(tag)`、`field_scalar_of` 返回 `Int`、`AstPattern::Enum` 收窄走整数比较、`compatible_with` 单向（标量枚举→整数，反向禁止）；放宽 `check_index`/`check_binary` 接受标量枚举作索引/位运算；`INTERFACE_VERSION` 1→2 使旧缓存失效。新增 `tests/run-pass/enum_scalar_context.{rl,out}` + `tests/compile-fail/enum_scalar_no_int_to_enum.rl`；`rlyeh test` 190 用例零回归 |
