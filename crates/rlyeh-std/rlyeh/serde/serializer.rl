// serde/serializer.rl：`Serializer`（序列化访问器框架）——2026-09-18 由 serde/module.rl 拆出。
//
// 归属子模块 `serde::serializer`。X3（2026-08-27）：持输出缓冲，访问器方法由手写
// `serialize` 实现调用，`result()` 取回拼接结果。用于用户自定义序列化（与编译器
// 内建 json/toml 并存）；完整 `serialize(&self, &mut Serializer) -> Result<(), SerError>`
// 目标签名待 SerError 接入。

struct Serializer {
    buf: String,
}

impl Serializer {
    fn new() -> Serializer {
        Serializer { buf: String::new() }
    }
    fn serialize_i64(&mut self, v: i64) {
        self.buf = self.buf + int_to_string(v);
    }
    fn serialize_str(&mut self, s: String) {
        self.buf = self.buf + s;
    }
    fn result(&self) -> String {
        self.buf
    }
}
