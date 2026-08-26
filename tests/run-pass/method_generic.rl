// U7：方法级泛型参数（fn map<U>）——不同 U 调用不碰撞（mono 键含方法泛型）
struct Marker { _u: i64 }
impl Marker {
    // 方法泛型 U：返回 U。不同 U（i64/f64/String）调用应各自单态化，
    // 不得复用首次 U（修复前 mono 键只含 impl 泛型导致碰撞）。
    fn map<U>(&self, x: U) -> U {
        x
    }
}

fn main() {
    let m = Marker { _u: 0 };
    println(m.map(10));                        // U=i64 → 10
    println(m.map(3.5));                       // U=f64 → 3.5
    println(m.map(String::from("hi")));        // U=String → hi
    // 嵌套调用：方法泛型 U 为 i64 的 map 结果再经加法
    let v = m.map(100);
    println(v + 1);                            // 101
}
