# SH-P1-3 嵌套模块 / `pub use` / 组与 glob 导入

> **级别**：P1（阻塞前端自举） · **状态**：🟢 部分完成（B1 既有 / B2·组导入·glob 已实现；B3 排除；B4 暂缓） · **归属**：0.2.0-B
> **索引**：[`../self-hosting-p1.md`](../self-hosting-p1.md) · **评估**：[`../../self-hosting/feasibility.md`](../../self-hosting/feasibility.md) §4 P1-6 · **计划**：[`../../development-plan-0.2.0.md`](../../development-plan-0.2.0.md) §3.2

## 目标
支持**嵌套模块**（既有）、`pub use` 重导出、组导入 `import a::{b, c}`、glob 导入 `import a::*`，
使 Rlyeh 侧能用层级化命名空间组织大型编译器 crate（替代当前扁平命名空间）。

> **B3 `super`/`crate::` 相对路径：经评估后排除**。依据 `docs/module-system.md` v1.1 决策，
> Rlyeh 采用**扁平名字空间**，路径首段恒为模块名，现实行 `模块名::Item` 编码，不引入
> `crate::`/`super::`/`self::`。故本阶段**不实现**相对路径语义（与 0.2.0 计划表 B3 文字
> 描述不同，以模块系统规范为准）。
>
> **B4 模块级可见性（默认私有 + `pub` 校验）：暂缓**。当前 std 全量依赖"跨模块全部可达"，
> 启用可见性校验需先为 std 全部跨模块引用补齐 `pub`，风险高、易引发大范围回归，本阶段不动。

## 技术细节
- 当前 Rlyeh 0.1.0：顶层 `module` + 内联/多文件嵌套模块（既有 ✅）+ 别名导入/重命名。
- 受影响 Rust 代码（事实依据，来自 `crates/` 核查）：
  - `rlyeh-driver/src/lib.rs:17-21` `pub mod error; pub mod incremental; mod module;`
  - `rlyeh-typecheck/src/check_expr/{mod,call,method,...}.rs`（32 文件多级模块）
  - `rlyeh-region-alloc/src/lib.rs:46-59` `mod block; mod bump; pub mod allocator; ...`
- 子任务（对应 0.2.0-B）：B1 嵌套模块（既有）/ B2 `pub use` / 组导入 / glob 导入 / B3 排除 / B4 暂缓。

## 受影响组件
parser（`parse_use`）、typecheck（`register_use` / `resolve_full_name` / `resolve_callable`）、
AST（`AstUseDecl` 增加 `group` / `is_pub`）。driver 模块展开不受影响（仅处理外部 `module foo;`）。

## 实现纪要（2026-09-02，本阶段交付）
**落地能力**
- **组导入** `import a::{b, c as d};`：`parse_use` 在 `::` 后识别 `{` 终止前缀，逐成员登记
  `local → a::member`（成员级 `as` 重命名）。单一 `AstUseDecl.group` 表示。
- **`pub use` 重导出** `pub import a::b as c;`：parser 分发层消费 `pub` 后传 `is_pub`；
  `register_use` 在 `is_pub` 时额外登记 `prefix::local → 目标全名`，使外部
  `import outer::revealed` 可经别名链 `r → outer::revealed → inner::secret` 解析到真实符号。
- **glob 导入** `import a::*;`：`register_use` 枚举 `a::` 直接子项（不含 `a::b::` 嵌套），
  逐一定位全名并登记；`pub import a::*` 同样重导出。

**核心机制改动**
- `AstUseDecl` 增加 `group: Option<Vec<(String, Option<String>)>>` 与 `is_pub: bool`。
- `register_use` 统一接收 `prefix` 参数（三处调用点：mod.rs / fn_sig.rs / collect.rs），
  按 group / glob / 简单 / `pub` 四类分支登记别名。
- `resolve_full_name` 改为**传递追踪**别名链（经 `use_aliases` 逐跳直到命中真实符号或回退
  模块前缀，`visited` 防环），并新增 `fn_templates` 参与 `is_direct` 判定——
  **关键修复**：泛型函数（如 `future::block_on`）注册在 `fn_templates` 而非 `fn_signatures`，
  原 `is_direct` 漏判会导致 `pub use` 链在泛型符号处断链。
- `resolve_callable`（`check_expr/resolve.rs`）兜底由单次 `use_aliases.get` 改为调用
  `resolve_full_name`，使函数调用亦支持多级 `pub use` 链（类型解析 `resolve_ast_type` /
  `resolve_named_type` 本就用 `resolve_full_name`，天然兼容）。

**测试**
- run-pass：`tests/run-pass/import_group.rl`（组导入）、`import_glob.rl`（glob）、
  `pub_use_reexport.rl`（`pub use` 多级链）→ 均 `[通过]`，全量套件 249 用例 0 失败。
- parser 单测：`test_use_group_and_pub` 锁定 AST 形状（`group` / `is_pub` / 简单路径）。

**已知限制（本次未解）**
- 嵌套组导入 `import a::{b::{x, y}, c}`（组内含子组）**已于 2026-09-04 补齐**（见下方「实现纪要（2026-09-04）」与变更记录）。
- 可见性（B4）未实现：跨模块访问仍不校验 `pub`，全部可达。
- `crate::`/`super::` 按扁平决策不实现。

## 验证
- 单元：组导入 / `pub use` 重导出 / glob 解析正确，多级链可跨模块访问。
- 验收：`cargo test --workspace` 全绿（含 `rlyeh_test_suite_all_pass` 全量 .rl 套件 0 失败）。

## 状态
🟢 部分完成：嵌套模块（既有）+ 组导入 + `pub use` + glob 已实现并通过；B3 排除、B4 暂缓。

## 变更记录
| 日期 | 变更 |
|------|------|
| 2026-09-01 | 从评估报告 P1-6 拆出为叶子 |
| 2026-09-02 | B1 嵌套（既有确认）/ B2 `pub use` / 组导入 / glob 实现；B3 按扁平决策排除；B4 暂缓；全量套件 0 失败 |
| 2026-09-04 | **补齐嵌套组导入** `import a::{b::{x, y}, c}`：`AstUseDecl.group` 改递归 `AstUseMember`；parser `parse_use_group` 递归解析任意深度子组；typecheck `register_use_group` 递归登记（仅叶子名入作用域，`pub` 重导出同构）；新增 `tests/run-pass/import_nested_group.{rl,out}` + parser 单测 `test_use_nested_group`；全量 `cargo test --workspace` 无回归。叶子「已知限制」中原「仅单层组」标记已消解 |
