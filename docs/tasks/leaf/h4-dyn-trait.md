# H4 `dyn Trait` trait 对象

> **所属阶段**：阶段 H
> **状态**：✅ 已完成
> **依赖**：G1
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

`dyn Trait` 类型 + `&T` 强制转换 + vtable 间接分派全链路。

## 背景

阶段 阶段 H 子任务，详见 阶段详情文档 [`stages/H.md`](../../stages/H.md)。

## 技术细节

布局 = 2 槽胖指针（槽 0 = 数据指针、槽 1 = vtable 指针）。`coerce_to_dyn`：`let d: dyn Shape = &c;` desugar 为 HIR 块——`Alloc(3+N)` vtable（槽 0–2 = drop/size/align 置 0；槽 3.. = FnPtr 按 trait 声明序）+ `Alloc(2)` 胖指针。`check_method_call` Dyn 分支：FieldGet(0) 数据指针 + FieldGet(1) vtable + Index(3+idx) 方法指针 + CallIndirect（多态）。MVP：trait/impl 非泛型；方法含 `Self` 不可 dyn 调用。

## 验证

`dyn_trait.{rlyeh,out}` 6 输出 + 全量 40 用例。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
