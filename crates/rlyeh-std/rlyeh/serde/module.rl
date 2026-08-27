// ---------------------------------------------------------------------------
// 阶段 Q1a / X3：Serialize / Deserialize trait 定义（std-lib.md §9 / 阶段 X）
//
// 语义说明（MVP）：
// - `json::stringify` / `json::parse::<T>` 为编译器内建（L2 ✅，std-lib.md §9），
//   struct 序列化（字段序 = 定义序）与反序列化（Q1c ✅，字段名匹配）经内建特判
//   desugar，不经过本 trait；
// - `Serialize` trait 供自定义类型手写 `impl Serialize for T { fn to_json(&self) .. }`；
//   内建类型（i64/bool/String）默认 impl 为声明性文档——MVP 内建类型的方法调用
//   不走 trait impl 查找（`x.to_json()` 报 `i64::to_json not found`），序列化统一
//   走 `json::stringify` 编译器特判；
// - `Deserialize`（X3 ✅，2026-08-27）：`-> Self` 返回已支持（U4），
//   手写 `impl Deserialize for T { fn from_json(s: String) -> Self; }` 可行
//   （见 tests/run-pass/x3_deserialize_trait.rl）；解析默认仍走编译器内建
//   `json::parse::<T>`（turbofish 定型）。
// ---------------------------------------------------------------------------

// 序列化 trait：`&self`（G1 ✅）+ String 返回（内建 `format!` 拼接）。
trait Serialize {
    fn to_json(&self) -> String;
}

// 反序列化 trait（X3，2026-08-27）：`-> Self` 返回已支持，手写 impl 接入。
trait Deserialize {
    fn from_json(s: String) -> Self;
}

// X3：序列化/反序列化错误类型（阶段 X 目标；`json::try_parse` 返回
// `Result<T, JsonError>` 的错误路径待内建解析器接入）。
enum JsonError {
    ParseError(String),
    InvalidType,
}

enum TomlError {
    ParseError(String),
    InvalidType,
}

// X3（2026-08-27）：Serializer 访问器框架基础——持输出缓冲，访问器方法由
// 手写 `serialize` 实现调用，`result()` 取回拼接结果。用于用户自定义序列化
//（与编译器内建 json/toml 并存）；完整 `serialize(&self, &mut Serializer) ->
// Result<(), SerError>` 目标签名待 SerError 接入。
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

// X3（2026-08-27）：Deserializer 访问器框架基础——持输入缓冲 + 位置游标，
// `deserialize_i64`/`deserialize_str`/`next_token` 读取访问器由手写 `deserialize`
// 实现调用，与 Serializer 对称（反向读取）。MVP 简化：读取以单字符分隔符
// （`:` / `,` / `}` / `]`）分段；完整 `deserialize(&mut self, &mut Deserializer) ->
// Result<T, DeError>` 目标签名待 DeError 接入。
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
                i = i;
            }
            self.pos = i;
        }
    }
}

// 内建类型默认 impl（声明性文档：MVP 序列化经 json::stringify 编译器特判）。
impl Serialize for i64 {
    fn to_json(&self) -> String {
        format!("{}", *self)
    }
}

impl Serialize for bool {
    fn to_json(&self) -> String {
        if *self { format!("true") } else { format!("false") }
    }
}

impl Serialize for String {
    fn to_json(&self) -> String {
        format!("\"{}\"", *self)
    }
}
