// PC-4：协议继承 `protocol Loud: Named`。
// 实现 Loud 的类型必须同时实现父协议 Named（声明点 `struct Dog: Loud, Named`）。

protocol Named {
    fn name_id(&self) -> i64;
}

protocol Loud: Named {
    fn shout(&self) -> i64;
}

struct Dog: Loud, Named {
    id: i64,
    fn name_id(&self) -> i64 { self.id }
    fn shout(&self) -> i64 { self.id * 10 }
}

fn main() {
    let d = Dog { id: 3 };
    println(d.name_id());   // 3
    println(d.shout());     // 30
}
