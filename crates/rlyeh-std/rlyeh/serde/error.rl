// serde/error.rl：序列化错误类型 —— 2026-09-18 由 serde/module.rl 拆出。
//
// 归属子模块 `serde::error`。X3 规划：`json::try_parse` 返回
// `Result<T, JsonError>` 的错误路径待内建解析器接入；`TomlError` 与
// `json::try_parse` 的 TOML 侧对称。

enum JsonError {
    ParseError(String),
    InvalidType,
}

enum TomlError {
    ParseError(String),
    InvalidType,
}
