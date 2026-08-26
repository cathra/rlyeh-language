# U6 数值转换 Cast IR

> **所属阶段**：阶段 U
> **状态**：✅ 已完成
> **依赖**：U5
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`as` 转换从静默擦除到完整落地。

## 背景

阶段 阶段 U 子任务，详见 阶段详情文档 [`stages/U.md`](../../stages/U.md)。

## 技术细节

typecheck Cast 分支仅对「数值→数值」且源/目标不同产出 HIR `Cast { expr, to }`（可转换标量 = ≤64 位整族 + 浮点 + bool + char，i128/u128 与指针/引用/聚合保持擦除）；MIR/LIR `Cast { target, value, to }` + LIR `cast_target_lir_type`；codegen 按源存储 + 目标语义位宽——fptosi/fptoui/sitofp/uitofp、trunc+sext/zext、icmp ne 0、add/fadd 0 恒等；全链分支补齐（DCE/内联/推断/names/borrowck/regionck）。修复两处 IR 生成 bug（`%%` 双百分号、裸名缺 `%`）。**解锁** `Duration::from_secs_f64`。

## 验证

`cast.{rlyeh,out}` 14 输出 + `duration_full` 增 from_secs_f64 + 全量 722 测试全绿。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
