# K3 `Rc<T>`/`Arc<T>` 引用计数装箱

> **所属阶段**：阶段 K
> **状态**：✅ 已完成
> **依赖**：K2
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

引用计数装箱：Rc/Arc 内建。

## 背景

阶段 阶段 K 子任务，详见 阶段详情文档 [`stages/K.md`](../../stages/K.md)。

## 技术细节

堆 `RcInner`（T 值区自堆首槽 + 尾部 strong/weak 计数槽），`Rc<T>` 栈 1 槽 Ptr。内建：`Rc::new`/`Arc::new`（FieldSet 计数初始化）、`clone`（强计数+1）、`strong_count`/`weak_count`、`downgrade`、`try_unwrap`、`Weak::upgrade`。接线：`check_rc_method` + `heap_ptr_hir` 统一 Box/Rc/Arc。关键教训：计数写必须用 FieldSet（GEP+store），`DerefSet` base 是地址。

## 验证

`rc_new.{rlyeh,out}` 17 输出 + rc_ty/rc_bad + 33 用例。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
