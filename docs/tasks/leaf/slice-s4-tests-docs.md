# S4 切片测试与文档

> **所属**：切片类型系统（规划 [`slice-type-system.md`](../slice-type-system.md)）
> **状态**：✅ 已完成（2026-08-30；`slice_basics` 用例全绿，规范三处已补切片章节）
> **阶段**：S4（测试与文档）

## 目标

`tests/run-pass` 切片用例 + `grammar.md`/`semantics.md` 补切片章节。

## 技术细节

- 用例：`&[i64]` 作函数参数/返回；数组/`Vec`/`String` 取切片 `&x[lo..<hi]`；切片索引/再切片/`.len()`；越界 clamp；Y1 `File::read(&mut [u8])` / `write(&[u8])`。
- `docs/grammar.md` 补 `[T]` 切片类型与生产式；`docs/semantics.md` 补切片类型规则与越界语义。

## 验收

- [x] 切片用例全绿：`tests/run-pass/slice_basics.{rl,out}`（185 用例全通过）。
- [x] 权威规范补切片章节，与 CODEBUDDY.md 一致。

## 已实现（2026-08-30）

| 位置 | 改动 |
|------|------|
| `tests/run-pass/slice_basics.rl` + `.out` | 切片综合用例：`&[i64]`/`&mut [i64]` 跨函数（非内联）传参、unsize coercion、索引、`iter()`、再切片、越界 clamp、`first`/`last`、`Vec<u8>` 切片视图、`File::read_slice` 二进制安全（含 NUL） |
| `docs/grammar.md` §2.4 | 补充 `[T]` 切片类型说明（DST、胖指针 2 槽布局、unsize coercion、支持的操作、MVP 限制）。生产式 `'[' Type ']'` 本已存在 |
| `docs/semantics.md` §8.3 | 新增「切片语义」章节（表示 / 构造 / 索引步长 / 再切片 clamp 规则 / 内建方法表 / MVP 限制 / `Vec<u8>` 紧凑与 `[u8; N]` 非紧凑的选型提示） |
| `CODEBUDDY.md` §数组与切片 | 补切片引用语法示例，并注明「数组范围切片返回 Vec 拷贝；切片接收者的范围切片返回零拷贝子区间」 |

## 变更记录

| 日期 | 变更 |
|------|------|
| 2026-08-30 | 由切片规划细化为叶子 |
| 2026-08-30 | 实现：`slice_basics` 用例（期望输出 `150/150/50/120/60/9/6/3/131`）；`grammar.md` §2.4、`semantics.md` §8.3、`CODEBUDDY.md` 补切片章节；`rlyeh test` 185 用例全通过 |
