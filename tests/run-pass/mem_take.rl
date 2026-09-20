// U（SH-P2-8 M3）：`mem::take(&mut a)` —— 返回 *a 旧值、原位留默认（Default::default()）。
// 依赖 `Default` 协议（`mem::take` 经 desugar 借 `let` 期望类型把 T 下传至 Default::default()）。
protocol Default { fn default() -> Self; }
impl i64: Default { fn default() -> Self { 0 } }
impl bool: Default { fn default() -> Self { false } }

struct Point { x: i64, y: i64 }
impl Point: Default { fn default() -> Self { Point { x: 0, y: 0 } } }

fn main() {
    let mut a = 5;
    let old = mem::take(&mut a);
    println(old);          // 5
    println(a);            // 0（默认）

    let mut p = Point { x: 1, y: 2 };
    let oldp = mem::take(&mut p);
    println(oldp.x);       // 1
    println(oldp.y);       // 2
    println(p.x);          // 0（默认）
    println(p.y);          // 0（默认）
}
