# 09 · 序列化：JSON / TOML

> 规范：docs/std-lib.md §9（序列化）
> 来源：复用 tests/run-pass 已通过回归的用例

## 功能点

- JSON：`json::stringify` / `json::parse::<T>`（内建别名 `json::to_string` / `json::from_str::<T>`）
  - 流式：`json::to_writer(w, v)` / `json::from_reader::<T>(r)`（配 File）
  - 覆盖：标量 / 数组 / struct / Vec / HashMap；反序列化 i64 / bool / String / HashMap
- `#[derive(Serialize, Deserialize)]` 派生宏
- TOML：`toml::to_string` / `toml::stringify` / `toml::from_str::<T>` / `toml::parse::<T>`
  - 标量 / 嵌套表（内联表）/ 数组 / HashMap；round-trip 反序列化

## 示例清单

| 文件 | 说明 |
|------|------|
| `json_api.rl` | Q2 JSON API：to_string/from_str + stringify/parse + to_writer/from_reader |
| `json_serde.rl` | Serialize / Deserialize protocol 序列化综合 |
| `json_derive.rl` | `#[derive(Serialize, Deserialize)]` 派生宏 |
| `toml_io.rl` | Q4 TOML：stringify / parse + 嵌套表 + Vec/HashMap round-trip |

## 运行

```bash
rlyeh run examples/std-demos/09-serialization/json_api.rl
rlyeh run examples/std-demos/09-serialization/json_serde.rl
rlyeh run examples/std-demos/09-serialization/json_derive.rl
rlyeh run examples/std-demos/09-serialization/toml_io.rl
```
