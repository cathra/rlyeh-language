# X4 Formatter 完整化

> **所属阶段**：阶段 X
> **状态**：📋 规划
> **依赖**：U3/U4、Q3
> **所属任务树**：[任务文档导航](../README.md) → [阶段 U–Z](../stage-u-z.md)

## 目标

`Formatter` 升级为真实格式化器 + `fmt -> Result<(), FmtError>`。

## 背景

阶段 阶段 X 子任务，详见 阶段详情文档 [`stages/X.md`](../../stages/X.md)。

## 技术细节

`Formatter` 从占位类型升级为真实格式化器（对齐/宽度/精度/填充状态字段）；`trait Display { fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>; }`（目标签名，依赖 U3/U4，替换 `-> String` 降级；`Debug::fmt_debug` 改名对齐 `Debug::fmt`）；`FmtError` 类型。

## 验证

`fmt_result.{rlyeh,out}`（`fmt -> Result` 对齐/宽度）。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-26 | 由阶段 U–Z 执行记录细化为独立叶子文档 |
