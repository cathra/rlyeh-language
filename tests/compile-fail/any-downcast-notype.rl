// G-M3：`any_downcast_ref` 必须显式指定目标类型（turbofish）——目标类型
// 决定待比对的 type_id，无法从实参推断（`dyn Any` 已擦除具体类型）。
// expect: 显式类型参数
struct Point { x: i64 }
fn main() {
    let p = Point { x: 1 };
    let a: dyn Any = &p;
    let r = any_downcast_ref(a);             // 缺少 ::<T>
    match r {
        Option::Some(pt) => println(pt.x),
        Option::None => println(0),
    }
}
