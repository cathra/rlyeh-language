// PC-3：多协议声明点一致性 + 「协议成员 vs 固有成员」裁决拆分。
// `struct Sq: Area, Named { .. }` → impl Area for Sq（area）+ impl Named for Sq（name_id）
//   + 固有 impl Sq（perimeter：未匹配任何协议需求）。

protocol Area {
    fn area(&self) -> i64;
}

protocol Named {
    fn name_id(&self) -> i64;
}

struct Sq: Area, Named {
    s: i64,
    fn area(&self) -> i64 { self.s * self.s }
    fn name_id(&self) -> i64 { 7 }
    fn perimeter(&self) -> i64 { self.s * 4 }
}

fn main() {
    let q = Sq { s: 3 };
    println(q.area());        // 9
    println(q.name_id());     // 7
    println(q.perimeter());   // 12（固有成员）
}
