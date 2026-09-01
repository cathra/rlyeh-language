// G-M2/G-M3（SH-P0-3）：`Any` 类型擦除 + `downcast` 安全检查。
// `&T → dyn Any` 把具体类型标识（type_id）存入 vtable 槽 0；
// `any_type_id(x)` 读回该标识，`any_downcast_ref::<T>(x)` 比对后产出
// `Option<&T>`——类型匹配为 `Some`，不匹配为 `None`。
struct Point { x: i64, y: i64 }
struct Msg { tag: i64 }

fn main() {
    let p = Point { x: 7, y: 8 };
    let m = Msg { tag: 99 };

    // 装箱：擦除为 `dyn Any`
    let a: dyn Any = &p;
    let b: dyn Any = &m;

    // type_id 往返：同一类型的标识一致，不同类型不同
    let id_p = any_type_id(a);
    let id_m = any_type_id(b);
    if id_p == any_type_id(a) {
        println(1);                  // 1：同值稳定
    } else {
        println(0);
    }
    if id_p != id_m {
        println(1);                  // 1：不同类型标识相异
    } else {
        println(0);
    }

    // downcast 成功：还原为 &Point 并读取字段
    let rp = any_downcast_ref::<Point>(a);
    match rp {
        Option::Some(pt) => println(pt.x),   // 7
        Option::None => println(-1),
    }

    // downcast 失败：请求的类型与实际不符 → None（安全，非未检查转换）
    let wrong = any_downcast_ref::<Msg>(a);
    match wrong {
        Option::Some(ms) => println(ms.tag),
        Option::None => println(0),           // 0
    }

    // 正确类型下 b 可还原为 &Msg
    let rm = any_downcast_ref::<Msg>(b);
    match rm {
        Option::Some(ms) => println(ms.tag),  // 99
        Option::None => println(-2),
    }
}
