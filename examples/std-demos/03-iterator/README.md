# 03 · 迭代器与适配器

> 规范：docs/std-lib.md §2.3（Iterator）
> 来源：复用 tests/run-pass 已通过回归的用例

## 功能点

- 自定义迭代器：实现 `next() -> Option<T>` 方法接入迭代协议
- `for x in it` 循环 desugar：`loop { match it.next() { Some(x) => body, None => break } }`
- 数组迭代：索引遍历（长度编译期已知）
- 适配器：`map` / `filter` / `fold` / `collect` / `take` / `skip`（返回 `Vec<T>` 可链式）

## 示例清单

| 文件 | 说明 |
|------|------|
| `iterator_trait.rl` | 自定义迭代器：`next() -> Option<i64>` + for 接入 |
| `adapters.rl` | J1–J3 适配器：map / filter / fold / collect / take / skip 链式 |
| `iterator_for.rl` | 自定义迭代器接入 `for` 循环 |
| `array_for.rl` | 数组字面量 for 迭代 |

## 运行

```bash
rlyeh run examples/std-demos/03-iterator/iterator_trait.rl
rlyeh run examples/std-demos/03-iterator/adapters.rl
rlyeh run examples/std-demos/03-iterator/iterator_for.rl
rlyeh run examples/std-demos/03-iterator/array_for.rl
```
