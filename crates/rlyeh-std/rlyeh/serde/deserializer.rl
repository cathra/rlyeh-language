// serde/deserializer.rl：`Deserializer`（反序列化访问器框架）——2026-09-18 由 serde/module.rl 拆出。
//
// 归属子模块 `serde::deserializer`。X3（2026-08-27）：持输入缓冲 + 位置游标，
// `deserialize_i64` / `deserialize_str` / `next_token` 读取访问器由手写 `deserialize`
// 实现调用，与 Serializer 对称（反向读取）。
//
// MVP 简化：读取以单字符分隔符（`:` / `,` / `}` / `]`）分段；完整
// `deserialize(&mut self, &mut Deserializer) -> Result<T, DeError>` 目标签名待 DeError 接入。

struct Deserializer {
    buf: String,
    pos: i64,
}

impl Deserializer {
    fn new(s: String) -> Deserializer {
        Deserializer { buf: s, pos: 0 }
    }
    // 读取一个 i64：截取从 pos 到分隔符（: , } ]）的子串，转整数，pos 前进到分隔符
    fn deserialize_i64(&mut self) -> i64 {
        let s = self.buf;
        let mut end: i64 = s.len();
        let mut i: i64 = self.pos;
        while i < s.len() {
            let ch = s.substring(i, i + 1);
            if ch == ":" || ch == "," || ch == "}" || ch == "]" {
                end = i;
                break;
            }
            i = i + 1;
        }
        let num_str = s.substring(self.pos, end);
        self.pos = end;
        string_to_int(num_str.trim())
    }
    // 读取一个字符串：截取从 pos 到分隔符（: , } ]），剥离首尾引号，pos 前进到分隔符
    fn deserialize_str(&mut self) -> String {
        let s = self.buf;
        let mut end: i64 = s.len();
        let mut i: i64 = self.pos;
        while i < s.len() {
            let ch = s.substring(i, i + 1);
            if ch == ":" || ch == "," || ch == "}" || ch == "]" {
                end = i;
                break;
            }
            i = i + 1;
        }
        let raw = s.substring(self.pos, end);
        self.pos = end;
        json_unescape(raw.trim())
    }
    // 跳过一个分隔符（或空白），推进 pos 到下一个值起始
    fn next_token(&mut self) {
        let s = self.buf;
        if self.pos < s.len() {
            let mut i: i64 = self.pos;
            while i < s.len() {
                let ch = s.substring(i, i + 1);
                if ch == ":" || ch == "," || ch == "}" || ch == "]" {
                    i = i + 1;
                } else {
                    break;
                }
            }
            self.pos = i;
        }
    }
}
