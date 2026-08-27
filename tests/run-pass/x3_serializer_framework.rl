// X3：Serializer 访问器框架（std serde/module.rl）
// `Serializer`（持输出缓冲 + serialize_i64/serialize_str + result()）+ 手写
// serialize 方法 + 泛型 `to_string<T: Serialize>`（Serialize::to_json 兼容并存）。
// 用户经 `serde::Serializer` 路径访问（std serde 模块命名空间）。

fn to_string_i64(v: i64) -> String {
    let mut ser = serde::Serializer { buf: String::new() };
    ser.serialize_i64(v);
    ser.result()
}

fn main() -> i64 {
    // 1. Serializer 访问器（serialize_i64）
    let mut ser = serde::Serializer { buf: String::new() };
    ser.serialize_i64(7);
    ser.serialize_str(String::from("-"));
    ser.serialize_i64(9);
    let r = ser.result(); // "7-9"

    // 2. 标量经访问器 + 泛型包装
    let r2 = to_string_i64(42); // "42"

    let mut total = 0;
    if r == "7-9" {
        total = total + 1;
    }
    if r2 == "42" {
        total = total + 1;
    }
    total
}
