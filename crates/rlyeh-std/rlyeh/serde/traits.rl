// serde/traits.rl：`Serialize` / `Deserialize` protocol —— 2026-09-18 由 serde/module.rl 拆出。
//
// 归属子模块 `serde::traits`。对外 `serde::Serialize` / `serde::Deserialize` 由
// serde/module.rl 的 `pub import serde::traits::*;` 登记重导出别名保持。
//
// 语义说明（MVP，std-lib.md §9 / 阶段 X）：
// - `json::stringify` / `json::parse::<T>` 为编译器内建（L2 ✅）：struct 序列化
//   （字段序 = 定义序）与反序列化（字段名匹配）经内建特判 desugar，不经过本 trait；
// - `Serialize`：供自定义类型手写 `impl Serialize for T { fn to_json(&self) .. }`；
//   内建类型（i64 / bool / String）的默认 impl 为声明性文档（见 impls.rl）；
// - `Deserialize`（X3 ✅，2026-08-27）：`-> Self` 返回已支持（U4），手写
//   `impl Deserialize for T { fn from_json(s: String) -> Self; }` 可行。

// 序列化 trait：`&self`（G1 ✅）+ String 返回（内建 `format!` 拼接）。
protocol Serialize {
    fn to_json(&self) -> String;
}

// 反序列化 trait（X3，2026-08-27）：`-> Self` 返回已支持，手写 impl 接入。
protocol Deserialize {
    fn from_json(s: String) -> Self;
}
