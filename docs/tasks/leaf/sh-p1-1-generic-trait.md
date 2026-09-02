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
| **A2** 泛型 impl | 待实现 | ⚠️ **基本已具备，两处缺口**。std 已用 `impl<T> Iterator for Iter<T>` / `impl<T> Future for RecvAsync<T>`。缺口见「已知限制」 |
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
- 全量 `cargo test --workspace -- --test-threads=1` 无失败（含 `.rl` 套件）。

## 已知限制（A2 的两处缺口，未在本轮处理）
1. **同一类型的同一泛型 trait 多 impl 无法按 trait 类型实参选择**——
   `impl Wrap<i64> for W` 与 `impl Wrap<bool> for W` 并存时，`w.wrap(true)` 会选中
   先注册的 `Wrap<i64>` 并报 `argument 1 expects i64, found bool`。
   根因：`ctx.find_impl_for_method` 为**首匹配即返回**，候选 impl 不参与实参类型
   比对。修复需把实参 `infer_expr` 提到候选选择之前（当前 subst 依赖 impl 签名，
   存在鸡生蛋），属对方法解析主路径的较大重构——`method.rs` 已 930 行，接近
   §7.0 上限，宜先按阶段下沉再改。
2. **impl 的类型参数只能由接收者类型 unify 推导**，trait 类型实参不参与绑定——
   故 `impl<T> Wrap<T> for W`（`W` 非泛型，且 `T` 仅出现在 trait 实参位置）报
   `undefined type T`；`impl<T> Wrap<T> for Pair<T>` 则正常（`T` 由 `Pair<T>` 绑定）。
   与缺口 1 同源（subst 的唯一来源是 `unify(impl_def.self_type, self_ty)`）。

## 状态
✅ 已完成（0.2.0 必须项，阶段 A）。A1 / A3 经复核为既有能力；A4 两处缺口已补；
A2 的两处限制见上，单独立项。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P1-4 拆出为叶子 |
| 2026-09-02 | 缺口复核（A1/A3 已具备、A2 剩两处限制、A4 两处真实缺口）；实现函数/方法级 `where` 子句与 impl 级约束强制校验；run-pass + 2 项 compile-fail 用例与全量回归 |
