// U2（2026-08-30）：受限制的类型联合——构造（成员 → 联合的向上转换）
// + `match` 类型臂收窄（tag 比较 + payload 按成员类型绑定）。
// 联合的运行时表示即匿名 enum（槽 0 = tag、槽 1 = payload），复用现有 enum codegen。
fn main() {
  // 成员 i64 → 联合：命中 i64 臂，payload 为 5
  let a: i64 | String = 5;
  match a {
    i64 => println(i64),      // 5
    String => println(-1),
  }
  // 成员 String → 联合：命中 String 臂，payload 为字符串 "hi"
  let s = String::from("hi");
  let b: i64 | String = s;
  match b {
    i64 => println(-1),
    String => println(String.len()),   // 2
  }
}
