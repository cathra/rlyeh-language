# S4 切片测试与文档

> **所属**：切片类型系统（规划 [`slice-type-system.md`](../slice-type-system.md)）
> **状态**：🔧 待实现
> **阶段**：S4（测试与文档）

## 目标

`tests/run-pass` 切片用例 + `grammar.md`/`semantics.md` 补切片章节。

## 技术细节

- 用例：`&[i64]` 作函数参数/返回；数组/`Vec`/`String` 取切片 `&x[lo..<hi]`；切片索引/再切片/`.len()`；越界 clamp；Y1 `File::read(&mut [u8])` / `write(&[u8])`。
- `docs/grammar.md` 补 `[T]` 切片类型与生产式；`docs/semantics.md` 补切片类型规则与越界语义。

## 验收

- [ ] 切片用例全绿。
- [ ] 权威规范补切片章节，与 CODEBUDDY.md 一致。

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-30 | 由切片规划细化为叶子 |
