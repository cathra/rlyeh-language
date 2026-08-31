// U4（2026-08-31）：字段级联合——struct 字段类型为 `A | B`。
// 覆盖：结构体字面量构造 / 字段读取 + match 类型臂收窄 / 字段赋值（=）/
// 多字段联合 / 与值级联合语义一致（U1/U2）。
struct S {
    id: i64 | String,
    tag: i64 | bool,
}

fn main() {
    // 构造：成员 i64 / bool → 字段联合
    let s = S { id: 5, tag: true };
    match s.id {
        i64 => println(i64),        // 5
        String => println(-1),
    }
    match s.tag {
        i64 => println(-1),
        bool => println(1),         // 1
    }

    // 构造：成员 String / i64 → 字段联合
    let mut s2 = S { id: String::from("hi"), tag: 7 };
    match s2.id {
        i64 => println(-1),
        String => println(String.len()),   // 2
    }

    // 字段赋值（=）：右值 i64 或 String 均可写入 `i64 | String` 字段
    s2.id = 42;
    match s2.id {
        i64 => println(i64),        // 42
        String => println(-1),
    }
    s2.id = String::from("z");
    match s2.id {
        i64 => println(-1),
        String => println(String.len()),   // 1
    }
}
