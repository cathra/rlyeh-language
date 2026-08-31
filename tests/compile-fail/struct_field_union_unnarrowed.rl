// U4（2026-08-31）：字段联合读取后须先 match 收窄，未收窄禁止直接运算。
// expect: expected a numeric type
struct S {
    id: i64 | String,
}

fn main() {
    let s = S { id: 5 };
    let x = s.id + 1;
    println(x);
}
