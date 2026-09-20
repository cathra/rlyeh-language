// SH-P1-1 A2（2026-09-02）边界补强：泛型 protocol 多 impl 选择的扩展覆盖。
//
// 在 generic_impl_multi.rl 基础上扩展：
//   - ① 同 self_type 三 impl（i64/bool/String），按 protocol 类型实参 / 实参选取
//   - ② 泛型 self_type `Pair<T>` + 泛型 impl 参数，T 由接收者推导（既有能力）
//   - ③ 模糊情形：精确 impl `Wrap<i64>` 与泛型 impl `Wrap<T>` 同时可用，
//        实参 i64 时优选具体 protocol 类型实参的 impl（确定性：105 = 100+5）。
// 同时压实「同 protocol 多 impl 单态化缓存键含 protocol_type_args」修复：
// 三 impl 方法体各异（i64 加 base / bool 恒等 / String 恒等），均被调用。

protocol Wrap<T> {
    fn wrap(&self, v: T) -> T;
}

// 三 impl 同 self_type：按 protocol 类型实参 / 实参选取
struct W3 { base: i64 }
impl W3: Wrap<i64> { fn wrap(&self, v: i64) -> i64 { v + self.base } }
impl W3: Wrap<bool> { fn wrap(&self, v: bool) -> bool { v } }
impl W3: Wrap<String> { fn wrap(&self, v: String) -> String { v } }

// 泛型 self_type + 泛型 impl 参数，由接收者推导
struct Pair<T> { a: T }
impl<T> Pair<T>: Wrap<T> { fn wrap(&self, v: T) -> T { v } }

// 模糊：精确 impl 与泛型 impl 并存
struct Amb { k: i64 }
impl Amb: Wrap<i64> { fn wrap(&self, v: i64) -> i64 { v + self.k } }
impl<T> Amb: Wrap<T> { fn wrap(&self, _v: T) -> T { _v } }

fn main() {
    let w3 = W3 { base: 1 };
    println(w3.wrap(10));                        // 11  (Wrap<i64>：10 + 1)
    println(if w3.wrap(true) { 1 } else { 0 });  // 1   (Wrap<bool>)
    println(w3.wrap(String::from("Z")).len());   // 1   (Wrap<String>)

    let p = Pair<i64> { a: 0 };
    println(p.wrap(33));                          // 33  (T=i64 由接收者推导)

    let a = Amb { k: 5 };
    println(a.wrap(100));                         // 105 (精确 impl Wrap<i64> 优先)
}
