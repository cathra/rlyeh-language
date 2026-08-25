# 12 · 宏：集合宏与声明式宏

> 规范：docs/std-lib.md §3.9a（集合宏）/ CODEBUDDY.md §3.9（宏）
> 来源：复用 tests/run-pass 已通过回归的用例

## 功能点

- 集合宏：`arr!`（数组字面量）/ `vec!`（Vec）/ `map!`（HashMap，`k => v` 元素）
- 声明式宏：`macro_rules!`（`$x:expr` / `$x:ident` / `$x:ty` / `$x:tt` 元变量 + `$(...)` 重复 `*`/`+`/`?`，parse 期递归展开）
- 元素为任意表达式：嵌套宏调用、绑定变量、完整表达式

## 示例清单

| 文件 | 说明 |
|------|------|
| `collection_macros.zeta` | I3 集合宏：arr! / vec! / map! + 空集合 + 嵌套表达式 |
| `macro.zeta` | macro_rules! 声明式宏基础 |
| `macro_expr_multi.zeta` | 多元素 / 重复模式 / 嵌套宏调用 |

## 运行

```bash
zeta run examples/std-demos/12-macros/collection_macros.zeta
zeta run examples/std-demos/12-macros/macro.zeta
zeta run examples/std-demos/12-macros/macro_expr_multi.zeta
```
