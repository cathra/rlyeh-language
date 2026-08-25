# 02 · 集合类型：Vec / String / HashMap

> 规范：docs/std-lib.md §3（集合类型）
> 来源：复用 tests/run-pass 已通过回归的用例

## 功能点

- `Vec<T>`：`vec![]` 字面量、`push` / `len` / `get` / `get_mut` / `iter` / `sort_by`（比较器闭包）
- `String`：构造、拼接、内容比较、按字符索引
- `HashMap<K, V>`：`map![]` 字面量、`insert` / `get` / `len` / 迭代
- `str` 值一等类型：字符串字面量 / 绑定字面量的变量直接调用方法

## 示例清单

| 文件 | 说明 |
|------|------|
| `vec_api.rl` | T1a Vec 目标 API：iter（拷贝缓冲）、get_mut、sort_by 升降序 |
| `string_api.rl` | String 常用 API 综合演示 |
| `hashmap_api.rl` | HashMap 插入 / 查询 / 遍历 / 匹配解构 |
| `str_value.rl` | str 值一等类型：方法调用 / `+` 拼接自动升级 String |

## 运行

```bash
rlyeh run examples/std-demos/02-collections/vec_api.rl
rlyeh run examples/std-demos/02-collections/string_api.rl
rlyeh run examples/std-demos/02-collections/hashmap_api.rl
rlyeh run examples/std-demos/02-collections/str_value.rl
```
