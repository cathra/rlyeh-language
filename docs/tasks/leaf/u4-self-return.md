# U4 `-> Self` 返回

> **所属阶段**：阶段 U
> **状态**：✅ 已完成
> **依赖**：U3
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

typecheck 支持 protocol/impl 方法签名返回 `Self`。

## 背景

阶段 阶段 U 子任务，详见 阶段详情文档 [`stages/U.md`](../../stages/U.md)。

## 技术细节

`TypeContext.self_type: Option<Type>` 上下文字段；`collect_impl` 方法签名解析时设为 impl 目标类型、`instantiate_impl_method` body 检查时设为 `substitute(self_type, subst)`；`resolve_named_type` 裸 `Self` 优先解析 self_type、protocol 上下文退化 `Generic("Self")`。覆盖 protocol/inherent/static 方法返回位置 + body 内 Self 注解 + 链式调用。MVP：`Self` 参数位置与 dyn 场景保持禁止。

## 验证

`self_return.{rlyeh,out}`（protocol/inherent/static + body 内 Self 注解 + 链式，输出 7/14/0/200/11/22）+ `self_ret_test.rs`。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
