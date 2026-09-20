# 阶段 U — 编译器地基（std 完整化前置）

> **所属任务树**：[任务文档导航](../tasks/README.md) → [阶段索引](../tasks/stage-u-z.md)
> **计划总览**：[`development-plan.md`](../development-plan.md) §2（计划总览）


> 现状：MVP 已验证 5 个能力缺口（见上表）。各任务实现细节、执行情况与技术细节见任务树对应叶子文档（[`stage-m-t.md`](../tasks/stage-m-t.md) / [`stage-u-z.md`](../tasks/stage-u-z.md)）。

| 任务 | 内容 | 状态 | 详情 |
|------|------|------|------|
| U1 | **作用域栈重构**：typecheck 变量环境 `variables: HashMap<String, Type>` → 按作用域分层（`Vec<HashMap>` 栈，块/函数/method/闭包/for 循环体各自压栈弹出），同名遮蔽按最近作用域解析；`local_inits` 同步分层；消灭 T 阶段已知限制（match 臂绑定与后续 let 同名互不覆盖）。**风险**：变量环境是全 typecheck 的共享状态（check_expr/check_item/check_stmt/closures 均读写），分层后所有读写点需走作用域 API；回归面大，须全量回归 | ✅ 已完成 | [`u1-scope-stack.md`](../tasks/leaf/u1-scope-stack.md) |
| U2 | **protocol 关联类型**：parser 支持 `protocol T { type Item; ... }`（`TypeMember` AST 节点，多条 type 声明允许）；typecheck 绑定 `Type::Assoc(name, protocol)` 解析（`Self::Item`/`Self::Output` 在 impl 实例化时替换为具体类型，unify 分支）；`Iterator::Item`/`Future::Output` 泛型化（§2.3/§10 目标 API 的前置）。**风险**：关联类型的解析/替换走通 protocol/impl 全链路，涉及 `substitute`/`unify` 递归；MVP 可先支持非泛型 protocol（复用 H4 约束） | ✅ 已完成 | [`u2-assoc-type.md`](../tasks/leaf/u2-assoc-type.md) |
| U3 | **泛型 protocol 约束（where / bound）**：parser 支持 `fn f<T: Bound>(...)` / `impl<K, V> HashMap<K, V> where K: Hash + Eq { ... }`；typecheck 记录约束并在调用点/实例化点校验；`T: Serialize`/`F: Future`/`B: FromIterator<T>` 签名落地。**风险**：约束推导（impl 的 where 与调用点约束联动）是核心复杂度，MVP 可先支持「声明 + 单 bound + 调用点宽松校验」（不推导） | ✅ 已完成 | [`u3-generic-bound.md`](../tasks/leaf/u3-generic-bound.md) |
| U4 | **`-> Self` 返回**：typecheck 支持 protocol 方法签名返回 `Self`（解析 `Self` 为当前 impl 的具体类型，`substitute` 替换）；落地 `Deserialize::from_json(s) -> Self`、`From::from(v) -> Self`、`Into::into() -> Self`。**风险**：`Self` 在参数位置（关联返回）与 dyn 场景（H4 限制）保持禁止；仅实现返回位置 | ✅ 已完成 | [`u4-self-return.md`](../tasks/leaf/u4-self-return.md) |
| U5 | **MIR `AddrOf` 任意目标表达式**：`AddrOf` 从「仅变量」扩展为任意表达式（先求值到临时槽再取址，与 MIR `lower_expr` 既有兜底同构）；落地 `Box::leak` 目标签名 `&'static mut T`（`&*b` 堆地址取引用）。**风险**：取临时地址的活跃性/DCE 正确性（H1 CallIndirect 曾踩坑），须补 MIR 测试 | ✅ 已完成 | [`u5-addr-of.md`](../tasks/leaf/u5-addr-of.md) |
| U6 | **数值转换 Cast IR**：`as` 转换在 typecheck 被**静默擦除**（`check_expr` Cast 分支直接返回 inner hir、类型改标 target，MIR/LIR/codegen 无转换指令——benchmark_report 已列为后续任务）；实现 HIR/MIR/LIR Cast 节点 + codegen 转换指令（`f64→i64` = `fptosi`、`i64→f64` = `sitofp`、整数截断/扩展 = `trunc`/`sext`/`zext`）；解锁 `Duration::from_secs_f64`（X1 注记项）+ mandelbrot 基准。**风险**：涉及 HIR→MIR→LIR→codegen 全链 lower + LLVM 指令映射，回归面大；HIR 仅对「数值→数值」Cast 产出节点（其余保持擦除，如指针/引用转换） | ✅ 已完成 | [`u6-cast-ir.md`](../tasks/leaf/u6-cast-ir.md) |

**验收**：见任务树 [`stage-u-z.md`](../tasks/stage-u-z.md) 各子任务叶子的「验证」字段；全量回归通过。
