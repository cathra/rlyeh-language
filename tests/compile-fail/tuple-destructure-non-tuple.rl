// M2：仅元组可解构——对标量解构应拒绝（无位置字段可取）。
// expect: 元组（2 元）
fn main() {
    let (a, b) = 42;          // i64 非元组
    println(a);
}
