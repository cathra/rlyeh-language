// SH-P1-1 A2（2026-09-02）：泛型 trait 多 impl 选择的两处限制修复。
//
// 此前（见 leaf 文档「已知限制」）：
//   ① 同一 self_type 上同一泛型 trait 的多个 impl（如 `impl Wrap<i64> for W` 与
//      `impl Wrap<bool> for W`）按首匹配选取，无法按 trait 类型实参 / 实参类型区分
//      —— `w.wrap(true)` 会选中 `Wrap<i64>` 并误报类型不匹配。
//   ② impl 的泛型参数只能由接收者类型 unify 推导，trait 类型实参不参与绑定 ——
//      `impl<T> Wrap<T> for W`（`W` 非泛型）报 `undefined type T`。
//
// 修复（check_method_call 重构）：收集全部候选 impl，按「代入 trait 类型实参后的
// 方法签名与实参类型兼容」选取首个匹配者；impl / 方法级泛型参数由对应实参反推。

trait Wrap<T> {
    fn wrap(&self, v: T) -> T;
}

// 限制①：同一 self_type 上同一泛型 trait 的多 impl，按 trait 类型实参 / 实参选取
struct W1 { base: i64 }

impl Wrap<i64> for W1 {
    fn wrap(&self, v: i64) -> i64 { v + self.base }
}

impl Wrap<bool> for W1 {
    fn wrap(&self, v: bool) -> bool { v }
}

// 限制②回归：impl 泛型参数由**接收者类型**推导（既有能力）
struct Pair<T> { a: T }

impl<T> Wrap<T> for Pair<T> {
    fn wrap(&self, v: T) -> T { v }
}

// 限制②：impl 泛型参数由**实参**推导（`W2` 非泛型，此前报 undefined type T）
struct W2 { base: i64 }

impl<T> Wrap<T> for W2 {
    fn wrap(&self, v: T) -> T { v }
}

fn main() {
    let w1 = W1 { base: 3 };
    println(w1.wrap(5));                        // 8   (选 Wrap<i64>：5 + 3)
    println(if w1.wrap(true) { 9 } else { 0 }); // 9   (选 Wrap<bool>)

    let p = Pair<i64> { a: 0 };
    println(p.wrap(11));                        // 11  (T 由接收者 i64 推导)

    let w2 = W2 { base: 0 };
    println(if w2.wrap(true) { 7 } else { 0 }); // 7   (T 由实参 bool 推导)
    println(w2.wrap(42));                       // 42  (T 由实参 i64 推导)
}
