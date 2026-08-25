// ---------------------------------------------------------------------------
// 阶段 Q1a：Serialize / Deserialize trait 定义（std-lib.md §9）
//
// 语义说明（MVP）：
// - `json::stringify` / `json::parse::<T>` 为编译器内建（L2 ✅，std-lib.md §9），
//   struct 序列化（字段序 = 定义序）与反序列化（Q1c ✅，字段名匹配）经内建特判
//   desugar，不经过本 trait；
// - `Serialize` trait 供自定义类型手写 `impl Serialize for T { fn to_json(&self) .. }`；
//   内建类型（i64/bool/String）默认 impl 为声明性文档——MVP 内建类型的方法调用
//   不走 trait impl 查找（`x.to_json()` 报 `i64::to_json not found`），序列化统一
//   走 `json::stringify` 编译器特判；
// - `Deserialize` 的 `-> Self` 返回自身类型未支持（typecheck undefined type
//   `Self`，见 io/error.rl M2b 注释），按 Q1a 预案退化为编译器内建
//   `json::parse::<T>`（turbofish 定型）；`from_json` 方法调用待 `Self` 返回支持。
// ---------------------------------------------------------------------------

// 序列化 trait：`&self`（G1 ✅）+ String 返回（内建 `format!` 拼接）。
trait Serialize {
    fn to_json(&self) -> String;
}

// 反序列化 trait：`-> Self` 返回自身类型未支持（见模块头注释），
// 解析统一经编译器内建 `json::parse::<T>`（L2 ✅）。规划中：
// trait Deserialize { fn from_json(s: String) -> Self; }

// 内建类型默认 impl（声明性文档：MVP 序列化经 json::stringify 编译器特判）。
impl Serialize for i64 {
    fn to_json(&self) -> String {
        format!("{}", *self)
    }
}

impl Serialize for bool {
    fn to_json(&self) -> String {
        if *self { format!("true") } else { format!("false") }
    }
}

impl Serialize for String {
    fn to_json(&self) -> String {
        format!("\"{}\"", *self)
    }
}
