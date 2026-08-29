# P6 泛型 trait 实参路径 + where 子句

> **所属专项**：[专项开发计划](../专项开发计划.md)（P6）
> **来源缺陷**：[`leaf/lang-defects.md`](lang-defects.md) #4（泛型 trait 实参不支持路径 + 无 where 子句，源自 Y6b）
> **状态**：📋 待办
> **风险**：中（原「中-高」——已通过 P6a-1~P6c-3 拆分为可独立实现/验证的子步骤；P6a 根因已定位 item.rs:368-374，P6b 复用已有 parse_where_clause（U3））
> **前置能力**：无

## 目标

泛型类型实参支持 `::` 路径（`impl From<io::error::IoErrorKind> for IoError`）+ 增加 `where` 子句（`impl<T, U> Into<U> for T where U: From<T>` blanket impl，`?` 运算符 From 自动转换前提）。

## 现状（2026-08-28 探测）

- **症状**：`impl From<io::error::IoErrorKind> for IoError` → parser 报 `expected '>', found Colon`；裸名 `impl From<IoErrorKind>` 可行
- **where 子句缺失**：`impl<T, U> Into<U> for T where U: From<T>`（blanket impl）不可行——无 `where` 子句语法
- **关键代码**：
  - `ty.rs:69-85` `parse_path_type` 已支持 `a::b::c` 路径 + 泛型参数（`Result<Response, Error>`）
  - 但 `impl From<io::error::IoErrorKind>` 报错——需定位 impl 头解析是否走完整 `parse_type`（疑似 impl 头特化路径未用完整 `parse_type`）
  - bound 解析 `item.rs:100-105` 仅 `expect_ident` 裸名，不支持 `::` 路径 bound

## 技术方案（拆分）

### P6a（中风险）impl 头泛型实参 `::` 路径

**根因已定位（2026-08-28）**：`item.rs:368-374` `parse_impl` 用 `expect_ident()` 解析 `impl` 后的第一个标识符——**只取裸名**，未走 `parse_type`/`parse_path_type`。`parse_type` 本身已支持 `::` 路径 + 泛型参数（`ty.rs:69-85`），但 impl 头解析绕过了它，导致 `impl From<io::error::IoErrorKind>` 中 `From` 后的 `::` 未被解析，报 `expected '>', found Colon`。

**细化子步骤**：
- **P6a-1（中风险）impl trait 实参走完整类型解析**：`parse_impl` 用 `parse_type` 替代 `expect_ident` 解析 trait 路径（`From<io::error::IoErrorKind>` 整体作为类型，含 `::` 路径 + 泛型实参）。验收：`impl From<io::error::IoErrorKind> for IoError` 语法通过。
- **P6a-2（中风险）impl trait 提取**：typecheck/后续阶段从解析出的 trait 类型中提取 trait 名 + 实参（区分 `From<T>` 与 `io::error::IoErrorKind` 实参）。验收：impl 绑定正确关联到 trait。
- **P6a-3（低风险）嵌套路径回归**：`impl Into<io::error::Result<i64>> for X` 等多层嵌套路径不报错。验收：回归全绿。

- **涉及**：`crates/rlyeh-parser/src/item.rs:368-374` + typecheck impl 解析
- **验收**：`impl From<io::error::IoErrorKind> for IoError` 可写且绑定正确

### P6b（中风险）`where` 子句语法

parser `item.rs` 增加 `where` 子句解析（`impl<T, U> Into<U> for T where U: From<T>` + 泛型函数 `fn f<T>(..) where T: Bound`）。

**现状**：`item.rs:122-128` `parse_where_clause` 已存在（U3），但仅按参数名合并 bound 到泛型参数列表，且 bound 解析 `item.rs:100-105` 仅 `expect_ident` 裸名，不支持 `::` 路径 bound。

- **P6b-1（中风险）`where` 子句 AST 承载**：确认 `parse_where_clause` 将约束合并到 `AstTypeParam`；新增**独立 where 约束列表**（非仅合并到参数）以支持 `where` 中出现未在泛型参数列表声明的类型（blanket impl 的 `U`）。验收：`impl<T, U> Into<U> for T where U: From<T>` 解析出 where 约束。
- **P6b-2（中风险）bound 支持 `::` 路径**：`item.rs:100-105` bound 从 `expect_ident` 升级为 `parse_path_type`，支持 `U: io::error::FromError` 路径 bound。验收：`where U: io::error::FromError` 可解析。
- **P6b-3（低风险）泛型函数 where**：`fn f<T>(..) where T: Bound` 在函数签名解析 where。验收：泛型函数 where 子句可解析进 AST。

- **涉及**：`item.rs` generics/impl 解析 + AST
- **验收**：`where` 子句语法（impl + 泛型函数）可解析进 AST，bound 支持 `::` 路径

### P6c（中风险）typecheck where 约束生效

where 约束在泛型 impl/函数实例化时检查。

- **P6c-1（中风险）where 约束收集**：typecheck 从 impl/函数签名收集 where 约束（trait bound + 路径 bound）。验收：`U: From<T>` 约束进入类型环境。
- **P6c-2（中风险）实例化校验**：泛型 impl 实例化时校验 where 约束（具体类型满足 `U: From<T>`），不满足报错。验收：`U: From<T>` 不满足时编译报错。
- **P6c-3（低风险）`?` 运算符 From 前提**：确认 blanket impl `Into<U> for T where U: From<T>` 使 `?` 运算符 From 自动转换可用。验收：`?` 从 `T` 自动 `Into<U>` 前提落地。

- **涉及**：typecheck 泛型约束检查
- **验收**：blanket impl 实例化时校验 `U: From<T>`；`?` 运算符 From 自动转换前提落地

## 执行步骤

1. 定位并修复 impl 头泛型实参走完整 `parse_type`（P6a-1→P6a-2→P6a-3）
2. 增加 `where` 子句语法解析（P6b-1→P6b-2→P6b-3）
3. typecheck where 约束生效 + bound 支持 `::` 路径（P6c-1→P6c-2→P6c-3）

## 验收标准（整体）

- 模块路径类型泛型 impl 可写
- where 子句 + blanket impl 生效

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-28 | 由专项开发计划 P6 生成叶子文档（拆分 P6a/b/c） |
| 2026-08-28 | 细化 P6a→P6a-1/2/3（根因 item.rs:368-374 expect_ident 不走 parse_type）、P6b→P6b-1/2/3、P6c→P6c-1/2/3，全部到可执行子步骤粒度 |
| 2026-08-28 | 整体风险降为中（子步骤均为中/低，P6a 根因已定位，P6b 复用 U3 parse_where_clause） |
