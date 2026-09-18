// PC-10：`dyn 子协议 → dyn 父协议` 静态上转（vtable 前缀兼容，零运行时开销）。
// 顺带验证：`dyn 子协议` 接收者可直接调用父协议方法（其槽位位于 vtable 前部）。
protocol Named {
    fn name_id(&self) -> i64;
}

protocol Loud: Named {
    fn shout(&self) -> i64;
}

struct Dog {
    x: i64,
}

impl Dog: Named {
    fn name_id(&self) -> i64 {
        11
    }
}

impl Dog: Loud {
    fn shout(&self) -> i64 {
        22
    }
}

fn main() {
    let d = Dog { x: 0 };

    // 具体类型 → dyn 子协议（H4 值上转）
    let l: dyn Loud = &d;

    // 线性化 vtable：[Named.name_id（槽 3）, Loud.shout（槽 4）]
    println(l.shout());      // 22（子协议方法）
    println(l.name_id());    // 11（父协议方法，槽位在前部）

    // `dyn Loud` → `dyn Named` 静态上转：复用同一胖指针
    let n: dyn Named = l;
    println(n.name_id());    // 11
}
