# Y5 `Box::leak` 目标签名

> **所属阶段**：阶段 Y
> **状态**：✅ 已完成（返回 `&'static mut T`，2026-08-28，166 用例全绿）
> **依赖**：U5
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`fn leak(self) -> &'static mut T`（替代裸指针退化）。

## 背景

阶段 阶段 Y 子任务，详见 阶段详情文档 [`stages/Y.md`](../../stages/Y.md)。

## 技术细节

`fn leak(self) -> &'static mut T`（依赖 U5 AddrOf 任意目标，替代 `*mut T` 裸指针退化）；`'static` 宽松丢弃（G4 现状）。

## 实施情况（2026-08-28）

- **call.rs** `Box::leak` 特判返回类型从 `Type::RawPtr(inner, true)`（`*mut T`）改为 `Type::Ref(inner, Mutability::Mutable)`（`&mut T`）。U5 AddrOf 已就绪——codegen 中引用与裸指针同为地址值（取 Box 槽 0 指针），`*leaked` 解引用走 `Type::Ref` 分支得 `T`，读写可用。
- 测试 `box_leak_ref.{rl,out}`：标量 `*p` 读取（42）、`*q` 写入（99）、`Box<Point>` 解引用拷贝（3）、`&mut i64` 显式注解（7）。现有 `box_leak.rl` 兼容（`*mut i64` 注解宽松接受）。

## 验证

`box_leak_ref.{rl,out}` + 现有 `box_leak.rl`（166 用例全绿）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
| 2026-08-28 | 标注 U5 AddrOf 依赖 + 风险评估 |
| 2026-08-28 | Box::leak 返回 &'static mut T 目标签名完成（166 用例全绿） |
