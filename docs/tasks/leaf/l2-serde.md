# L2 `serde` 序列化模块

> **所属阶段**：阶段 L
> **状态**：✅ 已完成
> **依赖**：I
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

`json::stringify`/`json::parse::<T>` 编译器内建，零新增 IR 节点。

## 背景

阶段 阶段 L 子任务，详见 阶段详情文档 [`stages/L.md`](../../stages/L.md)。

## 技术细节

**turbofish** `::<T>`：parser `check_turbofish()` + Call 后缀 `type_args` 字段。**stringify**（`json_serialize_ast`）：i64/bool/String（json_escape 转义）/数组/struct（字段序=定义序，递归）/Vec（while push_str）/HashMap（for 遍历，i64/String 键）。**parse**（`json_parse_ast`）：i64/bool/String/HashMap；缺 `::<T>` 报错。关键修复：Vec 与 struct 分支顺序 + guard、map! String 键、`"`→十六进制 `\22`、unify 多类型参数错误定型。

## 验证

`json_serde.{rlyeh,out}` 28 输出 + turbofish 单测 + 43 用例 + cargo test 678 通过。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
