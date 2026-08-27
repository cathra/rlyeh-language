// X3：Deserializer 访问器框架（std serde/module.rl）
// `Deserializer`（持输入缓冲 + 位置游标 + 读取访问器）与 Serializer 对称。
// 用户经 `serde::Deserializer` 字面量构造访问（`Deserializer::new` 静态方法在
// 模块中不解析，Rlyeh 模块静态方法限制——与 `Serializer::new` 一致）。

// 演示：用 Deserializer 手写反序列化一个 "7:9" 形式的紧凑文本
fn parse_pair(s: String) -> i64 {
    let mut de = serde::Deserializer { buf: s, pos: 0 };
    let a = de.deserialize_i64();
    de.next_token();
    let b = de.deserialize_i64();
    a + b
}

fn main() {
    // 1. Deserializer 字面量构造 + 读取访问器（deserialize_i64）
    let mut de = serde::Deserializer { buf: String::from("7:9"), pos: 0 };
    let a = de.deserialize_i64();
    de.next_token();
    let b = de.deserialize_i64();
    println(a + b); // 16

    // 2. 经函数封装的手写反序列化
    let r2 = parse_pair(String::from("10:20"));
    println(r2); // 30

    // 3. deserialize_str 读取（值含非数字，剥离引号）
    let mut de2 = serde::Deserializer { buf: String::from("name:\"rlyeh\""), pos: 0 };
    let tag = de2.deserialize_str();
    de2.next_token();
    let name = de2.deserialize_str();
    println(tag); // name
    println(name); // rlyeh
}
