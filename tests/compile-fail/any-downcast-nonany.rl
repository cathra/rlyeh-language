// G-M3：`any_downcast_ref` 仅接受 `dyn Any` 实参。对未擦除的具体类型引用
// 直接 downcast 应拒绝——否则可绕过 vtable 中的类型标识做伪转换。
// expect: dyn Any
struct Point { x: i64 }
fn main() {
    let p = Point { x: 1 };
    let r = any_downcast_ref::<Point>(&p);   // 未装箱为 dyn Any
    match r {
        Option::Some(pt) => println(pt.x),
        Option::None => println(0),
    }
}
