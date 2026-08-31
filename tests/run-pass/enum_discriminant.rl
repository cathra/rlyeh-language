// U3（2026-08-30）：受限标量枚举的**显式判别式** `Variant = 42`。
// 判别值在收集阶段写入 `VariantDef::tag`，构造与 `match` 的 tag 比较均复用该值，
// 故判别值与序号不一致时仍正确（IR 中为 `icmp eq i64 %x, 404` 而非序号）。
enum Code { Ok = 200, NotFound = 404, Error = 500 }

fn main() {
  let c = Code::NotFound;
  match c {
    Code::Ok => println(1),
    Code::NotFound => println(2),
    Code::Error => println(3),
  }
  let e = Code::Error;
  match e {
    Code::Ok => println(1),
    Code::NotFound => println(2),
    Code::Error => println(3),
  }
}
