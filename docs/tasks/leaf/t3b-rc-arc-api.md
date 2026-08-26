# T3b Rc/Arc 目标 API + `Weak::upgrade`

> **所属阶段**：阶段 T
> **状态**：✅ 已完成
> **依赖**：K3
> **所属任务树**：[任务文档导航](../README.md) → [阶段 M–T](../stage-m-t.md)

## 目标

Rc/Arc/Weak 目标 API 逐项核对。

## 背景

阶段 阶段 T 子任务，详见 阶段详情文档 [`stages/T.md`](../../stages/T.md)。

## 技术细节

逐项核对：`Rc::new`/`strong_count`/`weak_count`/`downgrade`/`try_unwrap`、`Arc::new`/`strong_count`/`weak_count`、`Weak::new`/`upgrade` 已全覆盖，无新增缺口。

## 验证

`rc_new.rl` 验收 + 全量回归。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 M–T 执行记录细化为独立叶子文档 |
