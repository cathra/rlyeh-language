// X2（2026-08-30）：TOML f64 序列化 / 反序列化 round-trip。
// - 裸浮点反序列化：`toml::from_str::<f64>("3.5")` → 3.5
// - 序列化后再反序列化：`toml::to_string(3.5)` → "3.5" → 3.5
fn main() -> i64 {
    let a = toml::from_str::<f64>("3.5");
    let s = toml::to_string(a); // "3.5"
    let b = toml::from_str::<f64>(s);
    println((a * 2.0) as i64); // 7
    println((b * 2.0) as i64); // 7
    0
}
