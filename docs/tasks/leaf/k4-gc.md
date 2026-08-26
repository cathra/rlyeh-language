# K4 `Gc<T>` 追踪 GC（MVP 保守标记-清除）

> **所属阶段**：阶段 K
> **状态**：✅ 已完成
> **依赖**：K3
> **所属任务树**：[任务文档导航](../README.md) → [阶段 G–L](../stage-g-l.md)

## 目标

编译器内建 + 独立运行时 `rlyeh-gc-runtime`。

## 背景

阶段 阶段 K 子任务，详见 阶段详情文档 [`stages/K.md`](../../stages/K.md)。

## 技术细节

布局：`Gc<T>` 栈 1 槽 Ptr → 堆 1-槽包装（槽 0 存 GcInner 基址）；对象 = slot_count(T) 连续堆块，与 Box 同构。生命周期协议：`gc_region` 块 desugar 为 begin（epoch+1）→ alloc（malloc + 注册块表 + 记录 epoch）→ escape（登记逃逸 root）→ collect（标记 → 清除 epoch 匹配未标记 → 存活提升 → epoch-1）。关键教训：escape 传对象 base、collect 存活提升须为所有 marked 对象、运行时只用 libc malloc/free 防 mfm_alloc 崩溃。

## 验证

`gc_region.{rlyeh,out}` 15 输出 + compile-fail/gc_bad.rl + 36 用例。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 G–L 执行记录细化为独立叶子文档 |
