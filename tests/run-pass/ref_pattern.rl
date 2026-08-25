// `ref` / `ref mut` 模式：绑定变量为对匹配值的引用（`&T` / `&mut T`）而非值拷贝。
// 覆盖：标量 match、String、枚举子模式、struct 字段访问、ref mut 解引用写、
// 语句级 `let ref x = e;`、聚合字段 ref（Some(String)）。
struct Point { x: i64, y: i64 }
fn main() {
    // 1. 标量 `ref r`：绑定 &i64，`*r` 解引用读
    let x = 42;
    match x {
        ref r => println(*r),       // 42
    }

    // 2. String `ref r`：绑定 &String，方法调用自动剥引用层
    let s = String::from("zeta");
    match s {
        ref r => println(r.len()),  // 4
    }

    // 3. 枚举子模式 `Some(ref v)`：对字段槽取引用
    let o = Option::Some(99);
    match o {
        Option::Some(ref v) => println(*v),   // 99
        Option::None => println(0),
    }

    // 4. struct 字段访问（引用自动剥层）
    let p = Point { x: 10, y: 20 };
    match p {
        ref r => println(r.x + r.y),          // 30
    }

    // 5. `ref mut r`：解引用写 + 读。MVP 中 match 先把匹配值拷贝到临时槽，
    //    `ref` 绑定指向该拷贝（与 Rust match ergonomics 直接绑定原值不同），
    //    写经引用作用于拷贝，原变量不受影响。
    let mut y = 7;
    match y {
        ref mut r => {
            *r = 100;
            println(*r);                      // 100：解引用写后经引用读
        }
    }
    println(y);                               // 7：匹配的是拷贝，原值不变

    // 6. 语句级 `let ref r = e;`
    let z = 5;
    let ref rz = z;
    println(*rz);                             // 5

    // 7. 聚合字段 ref（Some(String)）
    let o2 = Option::Some(String::from("hi"));
    match o2 {
        Option::Some(ref v) => println(v.len()),  // 2
        Option::None => println(0),
    }
}
