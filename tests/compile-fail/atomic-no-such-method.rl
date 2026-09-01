// H-M2：`AtomicI64` 不提供非原子访问器——原子值只能经 `load` / `store` /
// `fetch_*` / `compare_*` 访问（无 `get` 等绕过原子语义的便捷方法）。
// 本用例锁定该 API 边界（防止后续误加非原子读写入口）。
// expect: not found
fn main() {
    let a = AtomicI64::new(1);
    let v = a.get();     // 无 `get` 方法（原子值须经 load 读取）
    println(v);
}
