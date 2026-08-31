// U4（2026-08-30）：字段级联合——`struct` 字段类型可写 `A | B`。
// 构造与赋值时成员值均 desugar 为匿名 enum（复用 U2 的 `make_union_ctor`），
// 使字段槽存的是联合值（对象指针）而非裸成员值；读取后按 match 类型臂收窄。
struct S { id: i64 | String }

fn main() {
  // 构造时赋联合字段
  let a = S { id: 42 };
  match a.id {
    i64 => println(i64),      // 42
    String => println(-1),
  }
  let b = S { id: String::from("hi") };
  match b.id {
    i64 => println(-1),
    String => println(String.len()),   // 2
  }
  // 字段写入：判别值随赋值切换（i64 tag=0 → String tag=1）
  let mut c = S { id: 0 };
  c.id = 7;
  match c.id {
    i64 => println(i64),      // 7
    String => println(-1),
  }
  c.id = String::from("abc");
  match c.id {
    i64 => println(-1),
    String => println(String.len()),   // 3
  }
}
