# D1 `rlyeh test` 子命令

> **所属阶段**：阶段 D
> **状态**：✅ 已完成
> **依赖**：—
> **所属任务树**：[任务文档导航](../README.md) → [阶段 A–F](../stage-a-f.md)

## 目标

`rlyeh test` 测试框架。

## 背景

阶段 阶段 D 子任务，详见 阶段详情文档 [`stages/D.md`](../../stages/D.md)。

## 技术细节

`test_runner.rs`：`run_test_suite(root)` 扫描 compile-pass/compile-fail/run-pass 三目录——compile-pass 编译到 IR 成功即过；compile-fail 要求失败 + `// expect:` 断言错误消息；run-pass 编译运行 + `.out` 精确 stdout 对比；CLI `rlyeh test [<tests-dir>]`；`suite_test.rs` 断言全覆盖。连带修复①模块项 pub 可见性（parse_item 按关键字分派）②use 导入常量短名解析（`lookup_constant` 回退 use_aliases）。

## 验证

`rlyeh test tests` 13/13 全绿 + 全量回归通过。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 A–F 执行记录细化为独立叶子文档 |
