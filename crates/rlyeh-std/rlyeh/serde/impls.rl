// serde/impls.rl：内建类型的 `Serialize` 默认 impl —— 2026-09-18 由 serde/module.rl 拆出。
//
// 归属子模块 `serde::impls`（无对外导出，impl 块随协议定义注册）。
//
// 声明性文档（MVP）：序列化统一经 `json::stringify` 编译器特判，内建类型的方法调用
// 不走 protocol impl 查找（`x.to_json()` 报 `i64::to_json not found`）。
// 协议名 `Serialize` 经后缀兜底解析到 `serde::protocols::Serialize`。

impl i64: Serialize {
    fn to_json(&self) -> String {
        format!("{}", *self)
    }
}

impl bool: Serialize {
    fn to_json(&self) -> String {
        if *self { format!("true") } else { format!("false") }
    }
}

impl String: Serialize {
    fn to_json(&self) -> String {
        format!("\"{}\"", *self)
    }
}
