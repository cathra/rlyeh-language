// SH-P0-6：`if let` desugar 为 `match`，故继承 `match` 臂的模式支持范围——
// 元组 / 结构体模式在 match 位置尚不支持（可解构绑定 `let (a, b) = e;` 走
// check_stmt 的不可反驳绑定路径，不受此限）。须显式报错而非静默误匹配。
// expect: 元组 / 结构体模式
fn main() {
    let t = (3, 4);
    if let (a, b) = t {
        println(a + b);
    }
}
