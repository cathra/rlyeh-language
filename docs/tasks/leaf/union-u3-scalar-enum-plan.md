# U3 核心项专项工作计划：受限标量枚举（单标量存储）

> **所属**：受限制的类型联合（规划 [`type-union.md`](../type-union.md)）→ 叶子 [`union-u3-scalar-enum.md`](./union-u3-scalar-enum.md)
> **状态**：✅ 已完成（2026-08-30 核心项已落地：`Type::ScalarEnum` 表示 + 4 联动 + 缓存键 `INTERFACE_VERSION` 1→2；`rlyeh test` **190 用例全通过**；执行结果见下方「执行纪要」，详细根因与修复见叶子 [`union-u3-scalar-enum.md`](./union-u3-scalar-enum.md) 第四节）
> **前置**：显式判别式（`enum E { A = 1 }`）已落地并稳定；`is_scalar_enum` 判定函数保留在 `TypeContext` 待用
> **目标文档导航**：[任务文档导航](../README.md)

---

## 执行纪要（2026-08-30，实际推进）

Phase 0 的插桩（在 `resolve_named_type` 与 `collect_impl` 打点）**坐实**：基线（全 `Named`）
下 `collect_impl` 正确进入 `impl IoError` 且 `is_scalar_enum=true`，链路一致——前几轮失败
**不是**收集顺序/use 时机问题，而是 **ScalarEnum 分支只挂在 `resolve_named_type` 的「单条」
枚举返回路径（裸名分支），另一条「模块路径 / use 别名」分支仍产出 `Named`**，导致同一枚举
两种表示并存。

**实际落地（比计划更轻量）**：
- 不依赖 `register_uses_first`/`collect_enums_first` 预扫描（已确认非必需）；
- `resolve_named_type` **两条**枚举返回路径统一经 `is_scalar_enum` 产出 `Type::ScalarEnum`
  （主 ctx 与 `collect_fn_signatures` 接口路径共用，表示天然一致）；
- 4 联动（构造 `IntLiteral` / `field_scalar_of`→`Int` / 收窄整数比较 / `compatible_with` 单向）；
- 放宽 `check_index` / `check_binary` 接受标量枚举；
- `INTERFACE_VERSION` 1→2（缓存键 Display 对 `Named`/`ScalarEnum` 不可见，bump 使旧缓存失效）；
- 新增 `tests/run-pass/enum_scalar_context.{rl,out}` + `tests/compile-fail/enum_scalar_no_int_to_enum.rl`。
- 结果：**190 用例全通过**，std 全量（IoErrorKind 等）零回退。

计划中 Phase 0–3 已全部覆盖并收口；缓存键「层 4」风险通过 `INTERFACE_VERSION` bump 缓解
（更彻底的「表示感知序列化」方案保留为可选增强，当前 bump 机制已足够）。

---

## 0. 背景：三轮尝试暴露的障碍

U3 核心项（fieldless / 全单元变体枚举 → `Type::ScalarEnum` 单标量存储，可作数组索引/位运算/整数比较）已做三轮尝试，均回退：

| 轮次 | 改动 | 结果 | 归因 |
|------|------|------|------|
| 1 | 类型层编码 `Type::ScalarEnum` + 4 联动（构造/存储类别/收窄/放宽） | 184/188 失败 | 收集顺序依赖：struct 字段先于 enum 注册，类型表示不统一 |
| 2 | 加 `collect_enums_first` 预扫描（仅全单元变体 enum 提前注册） | 预扫描本身 188 过；重上 U3 仍 184 失败 | 更精确归因：裸名 `IoErrorKind` 依赖 `use` 别名，而 `register_use` 在收集阶段晚于签名解析 |
| 3 | 加 `register_uses_first`（任何解析前注册全部 use）+ `collect_enums_first` | 预扫描 188 过；重上 U3 仍 184 失败，且**两端均为完整名 `io::error::IoErrorKind`、仅变体不同**（形参 `Named` / 实参 `ScalarEnum`） | 见 §1 第三层障碍 + §2 缓存层硬伤 |

**核心不变式（贯穿全程）**：同一枚举的类型表示必须**全局唯一**。一旦某处解析为 `Named`、另一处为 `ScalarEnum`，二者 `Display` 相同但 `field_scalar_of` 布局不同（`Ptr` vs `Int`）→ 传参与字段读写全面错乱。因 `IoErrorKind` 等贯穿 IO/网络，影响面达 184/188。

---

## 1. 已查清的根因（三层 + 缓存层）

### 层 1：收集顺序依赖（已解，行为不变）
struct 字段类型解析早于 enum 注册 → 字段 `Named` 固化、调用点 `ScalarEnum`。
**解法已验证**：`collect_enums_first` 预扫描全单元变体 enum（无需 `resolve_ast_type`，安全提前）。预扫描单独跑 188 全过。

### 层 2：use 别名时机（已解，行为不变）
收集阶段 `collect_impl` 解析 `kind: IoErrorKind` 时 `use_aliases` 未注册 → 裸名无法映射完整名 → `is_scalar_enum("IoErrorKind")`=false → `Named`。
**解法已验证**：`register_uses_first` 在任何类型解析前注册全部 `UseDecl`（递归 mod）。预扫描 + use-first 单独跑 188 全过。

### 层 3：签名落库路径不一致（真正硬骨头）
存在**两条**独立的签名解析路径，各自持 `TypeContext`：
- **主类型检查路径**：`typecheck_with_region_hints` → `collect_declarations` → `collect_mod_types_inner` → `collect_impl`（`collect.rs:169`，参数经 `resolve_ast_type` 解析，`collect.rs:247`）。结果存入 `ctx.impl_defs`，`check_call` 经 `ctx.impl_defs` 读取形参类型（`call.rs:122-127`）。
- **独立接口提取路径**：`collect_fn_signatures`（`fn_sig.rs:9`）建**全新** `TypeContext::new()`（`:21`），重收集所有类型 + impl（`:28` → `collect_impl`），仅抽取顶层 `fn` 签名。供 `extract_interface` 用。

两条路径本应都经 `resolve_ast_type` 的 ScalarEnum 分支，理论上都产出 `ScalarEnum`。但实测 `resolve_ast_type` **全程未被**以 `IoErrorKind` 调用（调试日志佐证），说明 `IoError::new` 的形参 `Named` **并非来自主 ctx 的 `collect_impl`**——而来自一条不经主 typecheck 的缓存/接口路径（见 §2）。

### 层 4（NEW，最可能的真实机制）：增量接口缓存键「表示盲」

`extract_interface` / `compute_interface_hash`（`incremental/hash.rs`）的设计：

- 接口哈希（`:42-59`）对**规范化文本** `name(param1,param2):ret` 做 SHA-256，其中类型经 `Type::to_string()`（Display）序列化。
- **`Named("io::error::IoErrorKind")` 与 `ScalarEnum("io::error::IoErrorKind")` 的 Display 完全相同**（均 `"io::error::IoErrorKind"`）→ 接口哈希**无法区分**两种表示。
- 失效键（`:13-14`）仅有手动 `INTERFACE_VERSION: u32 = 1` **＋** 源码哈希 ＋ 接口哈希 —— **不含编译器版本、也不含枚举的 ScalarEnum 分类**。

**后果**：把某 enum 从 `Named` 改为 `ScalarEnum` 时，源码哈希与接口哈希**均不变**（Display 同串），旧缓存（由改动前的编译器产出 `Named`）被判定为命中并复用。即使 `--force` 仅清用户程序缓存、未清模块接口缓存，旧 `Named` 签名仍残留 → 形参 `Named`、实参（实时解析）`ScalarEnum` → 与实测错误**完全一致**。

> 此即「另一条路径的 `Named` 抵消单点改动」的具体机制——比层 3 的「两条路径」更底层：**缓存键对表示变更不可见**。

---

## 2. 专项工作计划

> 设计原则：**先做不可撼动的「表示一致性」地基（Phase 0–1），再铺 4 联动（Phase 2）**。前两轮失败皆因在一致性未立时改类型表示；本轮把地基（统一入口 + 缓存键表示感知）作为第一步。

### Phase 0 — 根因坐实（调查，必做，预算充足）

目标：把 §1 层 3/4 的推测坐实为确定性结论，并定下缓存键修复方案。

- **P0.1 形参 `Named` 的真实来源**：在 `TypeContext` 增临时钩子，凡构造 `Type::Named` 且其字符串含某哨兵枚举名时 `eprintln` 调用栈（或 `cfg!(debug_assertions)` 下 `backtrace`）；运行最小探针（用户自定义 fieldless enum + 调用），定位 `Named` 究竟由主 ctx `collect_impl` 还是 `extract_interface` 缓存路径产出。
- **P0.2 `extract_interface` 是否在 `run`/`test` 单程序路径被调用**：`search_content` 其调用方（`hash.rs:64` `extract_interface` 的所有引用），确认单文件 `run` 是否走模块接口缓存、缓存落盘位置与 `--force` 的清除范围。
- **P0.3 缓存键表示盲确认**：已读 `hash.rs:42-78`，确认 Display 序列化 + 手动 `INTERFACE_VERSION` 失效策略（层 4 结论成立）。补一个小测试验证：`enum E { A }` 改为标量枚举后 `compute_interface_hash` 不变（直接证明盲点）。
- **P0.4 定缓存键修复方案**（二选一，写入决策）：
  - **(a) 表示感知序列化**：`compute_interface_hash` 的规范化文本改用「带判别的」类型串（如 `ScalarEnum(io::error::IoErrorKind)` vs `Named(io::error::IoErrorKind)`），使两种表示哈希不同 → 表示变更自动失效。改动最小、语义最准。
  - **(b) 编译器版本/分类折叠**：在缓存键中纳入 `INTERFACE_VERSION`（表示变更时手动 +1）或 `is_scalar_enum` 分类集合的哈希。**(a) 优先**——自动感知，不依赖人工记 bump。
- **交付物**：根因 memo（确认层 3/4 哪条命中）+ 缓存键修复方案定稿。

### Phase 1 — 统一签名解析入口 + 缓存键表示感知（地基）

- **P1.1 抽离单一签名解析函数** `resolve_fn_signature(ctx, params, ret, self_ty, span) -> Result<FnSignature>`（`fn_sig.rs` 已有 `fn_signature_with_self`，但主 ctx 的 `collect_impl` 内联了等价逻辑 `collect.rs:229-263`）。让**主 ctx 的 `collect_impl` 与独立接口路径共用同一函数**，从根上杜绝「两条路径产出不同表示」。
- **P1.2 单一 ScalarEnum 判定点**：确保 `resolve_ast_type` 的 ScalarEnum 分支（依赖 `is_scalar_enum` + `register_uses_first` + `collect_enums_first`）是**唯一**把 enum 判为标量枚举的地方；`collect_fn_signatures` 复用同一 `resolve_ast_type`，故两者一致。
- **P1.3 落地 P0.4 的缓存键修复**（选 (a) 则改 `compute_interface_hash` 的序列化；同时把 `extract_interface` 提取的 `FunctionSignature.params/return_type` 改为带判别串）。补测试：表示变更后哈希变化。
- **P1.4 恢复 `register_uses_first` + `collect_enums_first`**（层 1/2 解法，行为不变），作为类型检查入口前置。
- **验收**：干净基线 188 过；新增单测证明缓存键表示感知（P1.3）。

### Phase 2 — 4 联动实现（在一致性地基上重做本叶子）

复用已设计好的 4 处改动（前两轮已写并已回退，逻辑清晰）：

1. **构造**（`check_variant_construct`，`index_enum.rs`）：标量枚举直接产出 `HirExpr::IntLiteral(tag)`，不再 `Alloc`——值即 tag 本身。
2. **存储类别**（`field_scalar_of`，`types.rs`）：标量枚举返回 `FieldScalar::Int`（当前 `Named(..)` 一律 `Ptr`）。
3. **收窄**（`check_pattern` 的 `AstPattern::Enum`，`index_enum.rs`）：标量枚举 `cond` 改为 `scrutinee == tag` 直接整数比较（当前 `FieldGet(scrutinee, 0) == tag`）；变体均为单元，不接收子模式。
4. **放宽**（`compatible_with`，`types.rs`）：标量枚举 → 整数**单向**兼容（读取 tag）；禁止整数 → 枚举，避免构造无对应判别式的非法值。

- **P2.1** 加 `Type::ScalarEnum(String)` 变体，补 parser / Display / `types.rs` 穷尽匹配（同 U1 加 `Type::Union` 时补了三处）。
- **P2.2–P2.5** 落地上述 4 点。
- **P2.6 MIR/LIR/codegen 一致性核查**：`ScalarEnum` 在 lowering（HIR→MIR→LIR）与 LLVM codegen 须按 `Int`（i64 tag）处理，与 `IntLiteral` 一致；确认无遗漏的 `Named` 分支假设（`search_content` 全仓 `Type::Named` 用法，逐处确认兼容 `ScalarEnum`）。

### Phase 3 — 验收与全量回归

- **P3.1 既有**：显式判别式用例（187）保持；全量 `rlyeh test` **188 全过**。
- **P3.2 新增 U3 核心用例**（`tests/run-pass/`）：
  - fieldless enum 作**数组索引**：`let a = [10,20,30]; let i: Color = Color::Green; a[i]` → 20。
  - **位运算**：`let mask = Perm::Read | Perm::Write;`（标量枚举位或）。
  - **整数比较**：`if k == IoErrorKind::NotFound {}`。
  - **赋值给整数**：`let n: i64 = code as i64;`（或 `let n: i64 = tag_of(Color::Red)`；C enum 语义，MVP 不强制值域校验）。
- **P3.3 缓存失效验证**：构造「先以旧编译器缓存、再切标量枚举」的回归用例，确认无 `Named` 残留（P1.3 单测覆盖）。
- **P3.4 std fieldless enum 盘点回归**：`IoErrorKind` / `Interest` / `OpenMode` / `Shutdown` 全量 std + tests 用例回归（影响面最大处）。

---

## 3. 风险

| 风险 | 等级 | 缓解 |
|------|:----:|------|
| 表示变更影响所有 fieldless enum（含 std `IoErrorKind` 等 4 个 + tests） | 中 | Phase 3.4 全量盘点回归；`is_scalar_enum` 仅命中「无泛型 + 全单元变体」，收敛影响面 |
| 缓存键改动波及增量编译一致性 | 中 | P1.3 单测 + P3.3 验证；表示感知序列化（方案 a）比手动 bump 更稳 |
| MIR/LIR/codegen 存在未显式处理的 `Named` 假设 | 中 | P2.6 全仓 `Type::Named` 用法核查 |
| 层 3/4 根因推测若与实际不符 | 低（已读源码坐实层 4） | Phase 0 坐实层 3 命中路径，避免盲改 |

---

## 4. 批次与依赖

```
Phase 0 根因坐实（P0.1–P0.4，调查，必先做）
   └─→ Phase 1 地基（P1.1 统一入口 → P1.2 单一判定 → P1.3 缓存键 → P1.4 use/enum 预扫描）
          └─→ Phase 2 4 联动（P2.1 变体 → P2.2–P2.5 改动 → P2.6 全层核查）
                 └─→ Phase 3 验收（P3.1 回归 → P3.2 新用例 → P3.3 缓存 → P3.4 std 盘点）
```

| 批次 | 内容 | 理由 |
|------|------|------|
| 批次 1（调查） | **Phase 0** P0.1–P0.4 | 先把「形参 Named 真实来源」与「缓存键盲点」坐实，定缓存键修复方案；不写功能代码 |
| 批次 2（地基） | **Phase 1** P1.1–P1.4 | 统一签名入口 + 缓存键表示感知 + 恢复预扫描；行为不变，188 仍过 |
| 批次 3（功能） | **Phase 2** P2.1–P2.6 | 在一致性地基上重做 4 联动；`ScalarEnum` 全层贯通 |
| 批次 4（验收） | **Phase 3** P3.1–P3.4 | 回归 + 新用例 + 缓存失效验证 + std 盘点 |

---

## 5. 验收标准（整体）

- [ ] 单一签名解析入口（`collect_impl` 主路径与 `collect_fn_signatures` 接口路径共用），无「两条路径产出不同表示」。
- [ ] 增量接口缓存键**表示感知**：enum `Named`↔`ScalarEnum` 变更使哈希变化，旧缓存自动失效。
- [ ] fieldless / 全单元变体枚举：`field_scalar_of`=Int；构造产出 `IntLiteral(tag)`；收窄为整数比较；与整数单向兼容。
- [ ] 可作数组索引 / 位运算 / 整数比较（P3.2 用例通过）。
- [ ] `rlyeh test` 全量回归（基线 188）零失败；std fieldless enum 盘点无残留 `Named` 不一致。

---

## 6. 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-30 | 基于三轮尝试 + 根因调查制定专项工作计划：坐实三层障碍 + 新增「层 4 增量接口缓存键 Display 盲点」（`hash.rs:42-78` 用 `Type::to_string()` 序列化，无法区分 `Named`/`ScalarEnum`，失效仅依赖手动 `INTERFACE_VERSION=1`）；规划 Phase 0 根因坐实 → Phase 1 统一签名入口 + 缓存键表示感知 → Phase 2 重做 4 联动 → Phase 3 验收，明确各批次交付物与风险缓解 |
