// PC-4：实现协议 `Loud` 必须同时实现其父协议 `Named`（此处缺失 → TC029）。
// expect: superprotocol

protocol Named {
    fn name_id(&self) -> i64;
}

protocol Loud: Named {
    fn shout(&self) -> i64;
}

struct Bad: Loud {
    id: i64,
    fn shout(&self) -> i64 { self.id }
}

fn main() {
    let b = Bad { id: 1 };
    println(b.shout());
}
