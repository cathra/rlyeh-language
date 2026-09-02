# SH-P1-1 泛型 trait / impl 完整化

> **级别**：P1（阻塞前端自举） · **状态**：✅ 已完成（A4 收尾；A1–A3 经复核已具备） · **归属**：0.2.0-A
> **索引**：[`../self-hosting-p1.md`](../self-hosting-p1.md) · **评估**：[`../../self-hosting/feasibility.md`](../../self-hosting/feasibility.md) §4 P1-4 · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.1

## 目标
支持**带泛型参数的 trait 声明与 impl**，并允许 impl 方法返回 `Self`，解除"注释 typecheck 自身需泛型 trait"的鸡生蛋问题，解锁前端自举。

## 技术细节
- 当前 Rlyeh 0.1.0：「trait+impl 必须非泛型」（见 `docs/guide/13-references-limits.md`）。关联类型已在 U2 落地，本任务在其上叠加泛型参数化。
- 受影响 Rust 代码（事实依据）：`rlyeh-typecheck/src/types.rs:440` `impl Trait<Args> for Type` 带泛型实参列表；`check_expr/generic.rs`（16.6K）、`closure.rs`（26K）依赖泛型实例化。
- 子任务（对应 0.2.0-A）：A1 泛型 trait 声明 / A2 泛型 impl / A3 含 `Self` 返回的 impl 方法 / A4 泛型约束 `where`/`:` 收尾。

## 受影响组件
`rlyeh-typecheck`（trait/impl 解析、泛型实例化）、前端所有需泛型 trait 的 IR 节点。

## 缺口复核（2026-09-02，动手前）
叶子「技术细节」称「当前 Rlyeh 0.1.0：trait+impl 必须非泛型」——**该判断已过时**。
实测四项子任务的真实状态：

| 子任务 | 规划描述 | 复核结果 |
|--------|---------|---------|
| **A1** 泛型 trait 声明 | 待实现 | ✅ **已具备**（U8）。std 已用 `trait From<T>` / `trait Into<T>`（`io/error.rl`）；测试有 `trait Wrap<T>`（`generic_ctor_trait.rl`）、`trait Deref<T>`（`y4b_deref.rl`）。parser `parse_trait` 早接 `parse_generics` |
| **A2** 泛型 impl | 待实现 | ✅ **已具备（含两处限制已修复，2026-09-02 第二轮）**。std 已用 `impl<T> Iterator for Iter<T>` / `impl<T> Future for RecvAsync<T>` |
| **A3** 含 `Self` 返回 | 待实现 | ✅ **已具备**（SH-P0-3 G-M1 的 `replace_type_self` + 按值 `Self` 返回 codegen 修复）。std `error_conversion.rl` 即 `trait From<T> { fn from(v: T) -> Self; }` |
| **A4** 约束 `where`/`:` | 待收尾 | ❌ **两处真实缺口**：① **函数级 `where` 完全缺失**——尽管 `grammar.md` §2.3 早已写入 `FnDecl ::= ... RetType? WhereClause? Block`，`parse_fn` 却未接 `parse_where_clause`，函数声明遇 `where` 报语法错误；② **impl 级约束（内联 `<T: B>` 与 `where` 两种写法）均只记录不校验**——`ImplDef.bounds` 的文档注释自陈「MVP 记录不校验」 |

## 实现纪要（2026-09-02）

### A4-① 函数 / 方法级 `where` 子句
`parse_fn` 在返回类型之后、函数体之前接入既有 `parse_where_clause`（此前仅
`parse_impl` 调用它）。约束按参数名合并进 `AstFnDecl.generics`，从而复用与内联
bound **完全相同**的校验路径与诊断（`GenericBoundMismatch`）。
覆盖：函数、impl 块内方法、trait 抽象方法声明。

### A4-② impl 块级约束强制校验
在 `check_method_call` 调用点实例化方法体之前（`instantiate_impl_method` 之前）
调用既有 `check_generic_bounds(ctx, &impl_def.bounds, &subst, span)`。
此前违反约束时**不在调用点报错**，而是直接实例化方法体、在体内部报出误导性的
`function i64::speak not found`；现在给出与函数级一致的诊断：
``type `i64` does not implement trait `Speak` (bound on generic parameter `T`)``。
`ImplDef.bounds` 的「MVP 记录不校验」注释同步更新。

## 验证
- `tests/run-pass/generic_where_clause.{rl,out}`（5 行输出）：函数级 where 单约束 /
  where 多约束 / **内联 bound 与 where 混用** / impl 块内方法 where / impl 块级
  where（既有能力回归）。
- `tests/compile-fail/generic-where-fn-bound.rl`：函数级 where 约束强制。
- `tests/compile-fail/generic-where-impl-bound.rl`：impl 级约束强制（修复前报的是
  体内部的 `i64::speak not found`）。
- `tests/run-pass/generic_impl_multi_ext.{rl,out}`（5 行输出，2026-09-02 补测）：A2
  扩展覆盖——① 同 self_type **三 impl**（i64/bool/String）按 trait 类型实参 / 实参
  选取；② 泛型 self_type `Pair<T>` + 泛型 impl 参数，T 由**接收者**推导；③ 模糊情形
  （精确 impl `Wrap<i64>` 与泛型 impl `Wrap<T>` 并存）实参 i64 时**优选具体 trait
  类型实参的 impl**（确定性 105 = 100+5）。同时压实「同 trait 多 impl 单态化缓存键
  含 `trait_type_args`」修复（三 impl 方法体各异，均被调用）。
- `tests/run-pass/generic_impl_where_arg.{rl,out}`（1 行输出，2026-09-02 补测）：
  A2②（impl 泛型由**实参**反推）+ A4（impl 级 `where`）协同生效的干净路径
  （`T=IntN` 由实参反推并满足 `where T: Num` → 107）。
- 全量 `cargo test --workspace -- --test-threads=1` 无失败（含 `.rl` 套件）。

## 已知限制（A2 两处已修复，2026-09-02 第二轮）

> 此前记录的两处限制已在本轮修复，记录如下供追溯。

1. **同一类型的同一泛型 trait 多 impl 无法按 trait 类型实参选择**——
   `impl Wrap<i64> for W` 与 `impl Wrap<bool> for W` 并存时，`w.wrap(true)` 会选中
   先注册的 `Wrap<i64>` 并报 `argument 1 expects i64, found bool`。
   **根因（双重）**：① `ctx.find_impl_for_method` 为**首匹配即返回**，候选 impl 不参与
   实参类型比对；② 即使选对 impl，`instantiate_impl_method` 的 mono 键只含
   `impl_def.type_params`（此处为空），**未含 `trait_type_args`**，导致两个 impl 的
   mono 键完全相同 → `mono_instances` 缓存命中复用首个实例，方法体 / 接收者错配
   （表现为跨调用结果异常：`w1.wrap(5)` 在 `w1.wrap(true)` 之后返回 5 而非 8）。
   **修复**：`check_method_call` 改为收集全部候选 impl，按「代入 trait 类型实参后
   的方法签名与实参类型兼容」选取首个匹配者；`instantiate_impl_method` 的 mono 键 /
   后缀并入 `trait_type_args`（经 subst 代入后）。
2. **impl 的类型参数只能由接收者类型 unify 推导**，trait 类型实参不参与绑定——
   故 `impl<T> Wrap<T> for W`（`W` 非泛型，且 `T` 仅出现在 trait 实参位置）报
   `undefined type T`；`impl<T> Wrap<T> for Pair<T>` 则正常（`T` 由 `Pair<T>` 绑定）。
   **修复**：选取候选时，将 impl / 方法级未定泛型参数（`cand.type_params` ∪ 方法
   泛型）由对应实参类型 `unify` 反推绑定，使 `impl<T> Wrap<T> for W` 的 `T` 可由
   实参推导。

3. **`where` 泛型 impl 与精确 impl 并存时未回退**（A2+A4 交集，2026-09-02 补测发现）：
   同 self_type 同时存在「精确 impl `Convert<i64>`」与「带 `where` 的泛型 impl
   `Convert<T> where T: Num`」时，对 `c.convert(5)`（i64）编译器会就泛型 impl 报
   `type i64 does not implement trait Num`，而未回退到完全匹配的精确 impl（Rust 会
   选精确 impl 返回 105）。**根因**：候选选择按「代入 trait 类型实参后的方法签名与
   实参兼容」选取，但 `where` 约束的校验发生在选定候选之后的实例化阶段，未作为
   「该候选是否适用」的过滤条件；`i64: Num` 不满足时直接报错而非跳过该候选。
   **当前规避**：测试用例刻意只保留带 `where` 的泛型 impl（见
   `generic_impl_where_arg.rl`），不与精确 impl 冲突；该限制待候选评分纳入 `where`
   可行性后再消解（不影响既有 impl 级 `where` 单 impl 场景，已由
   `generic_where_clause.rl` / `generic_impl_where_arg.rl` 覆盖）。

## 状态
✅ 已完成（0.2.0 必须项，阶段 A）。A1 / A3 经复核为既有能力；A4 两处缺口已补；
A2 两处限制（多 impl 按实参选择 + impl 泛型由实参推导 + 同 trait 多 impl 单态化
缓存碰撞）均已于 2026-09-02 第二轮修复。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P1-4 拆出为叶子 |
| 2026-09-02 | 缺口复核（A1/A3 已具备、A2 剩两处限制、A4 两处真实缺口）；实现函数/方法级 `where` 子句与 impl 级约束强制校验；run-pass + 2 项 compile-fail 用例与全量回归 |
| 2026-09-02 | **A2 两处限制修复（第二轮）**：① 方法解析改为收集全部候选 impl，按「代入 trait 类型实参后的方法签名与实参兼容」选取首个匹配者（`check_method_call` 重构 + `find_impl_candidates` / `find_trait_method_candidates`）；② impl / 方法级泛型参数由对应实参反推绑定；③ `instantiate_impl_method` 的 mono 键 / 后缀并入 `trait_type_args`，修复同 trait 多 impl 单态化缓存碰撞（此前 `w1.wrap(5)` 在 `w1.wrap(true)` 之后返回 5 而非 8）。新增 `generic_impl_multi.rl`（5 行输出）+ `generic-impl-mismatch.rl` 门禁，全量套件通过 |
