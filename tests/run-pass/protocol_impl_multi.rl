// PC-9：`impl T: A, B` 多协议一致性——desugar 按协议成员名裁决拆分为多个 impl 块。
protocol Area {
    fn area(&self) -> i64;
}

protocol Named {
    fn name_id(&self) -> i64;
}

struct Sq {
    s: i64,
}

// 一致性列表含两个协议：area 属 Area、name_id 属 Named；
// perimeter 不属任何协议，并入首个协议块（impl Sq: Area）。
impl Sq: Area, Named {
    fn area(&self) -> i64 {
        self.s * self.s
    }
    fn name_id(&self) -> i64 {
        7
    }
    fn perimeter(&self) -> i64 {
        self.s * 4
    }
}

fn main() {
    let q = Sq { s: 3 };
    println(q.area());       // 9（Area 块）
    println(q.name_id());    // 7（Named 块）
    println(q.perimeter());  // 12（并入 Area 块的方法仍可按类型解析）
}
