// SH-P1-1 A2（2026-09-02）：多 impl 按实参选取后，实参类型仍不匹配时须报出
// 清晰的类型不匹配诊断，而非静默误用首个 impl。
// expect: expects `bool`
protocol Wrap<T> {
    fn wrap(&self, v: T) -> T;
}

struct W1 { base: i64 }

impl W1: Wrap<bool> {
    fn wrap(&self, v: bool) -> bool { v }
}

fn main() {
    let w = W1 { base: 3 };
    println(w.wrap("nope"));
}
