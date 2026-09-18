// SH-P1-1 A2② + A4 交集（2026-09-02）：impl 泛型参数由**实参**反推，且 impl 带
// 级 `where` 约束。与 generic_where_clause.rl 的区别：彼处 `T` 由**接收者类型**
// 推导（`Pair<T>`），此处 `T` 由**方法实参**推导（W2 非泛型），并叠加 `where T: Num`
// 约束——验证 A2② 的反推与 A4 的 impl 级约束在该路径上协同生效。
//
// 注：若同时存在「精确 impl `Convert<i64>`」与「带 where 的泛型 impl `Convert<T>
// where T: Num`」，对 `convert(5)`（i64）当前会就泛型 impl 报 `i64 does not
// implement Num` 而未回退到精确 impl（候选选择未因 where 不满足而过滤该 impl）。
// 故本用例刻意只保留带 where 的泛型 impl，避免该限制触发。

protocol Convert<T> { fn convert(&self, v: T) -> i64; }
protocol Num { fn val(&self) -> i64; }

struct IntN { v: i64 }
impl IntN: Num { fn val(&self) -> i64 { self.v } }

struct C { k: i64 }
impl<T> C: Convert<T> where T: Num {
    fn convert(&self, v: T) -> i64 { v.val() + self.k }
}

fn main() {
    let c = C { k: 100 };
    let n = IntN { v: 7 };
    println(c.convert(n));   // 107  (T=IntN 由实参反推，满足 where T: Num)
}
