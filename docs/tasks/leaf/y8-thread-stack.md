# Y8 `thread::Builder::stack_size`

> **所属阶段**：阶段 Y
> **状态**：✅ 已完成
> **依赖**：S0e
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`Builder::new()/stack_size(bytes)/spawn(f)` 定制线程栈。

## 背景

阶段 阶段 Y 子任务，详见 阶段详情文档 [`stages/Y.md`](../../stages/Y.md)。

## 技术细节

`thread/module.rl` 增加 `Builder`（`new`/`stack_size(&mut self, n)`/`start(&self, f)`）+ driver 注入 `__rlyeh_thread_spawn_stack`（attr 定制，n<=0 → attr=NULL 等价 `Thread::start`）；修复 codegen by_value 逃逸 bug——聚合对象地址经 FieldSet 嵌入堆对象时栈槽悬垂，预扫描逃逸诊断回退 calloc。

## 验证

`thread_test.rs` 新增 `builder_default_stack_spawn`/`builder_stack_size_deep_recursion`（64MB 栈 100000 层深递归，默认栈 SIGSEGV 反证定制生效）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
