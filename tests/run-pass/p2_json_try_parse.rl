// P2（2026-08-28）：`json::try_parse::<T>(s) -> Result<T, serde::JsonError>` 严格解析。
// 合法输入返回 Ok，非法输入返回 Err(JsonError::ParseError)——替代 `json::parse` 的
// 静默零值/宽松解析。覆盖 i64 / bool / String 三类标量的合法与非法输入。

fn main() {
    // 合法 i64
    match json::try_parse::<i64>(String::from("42")) {
        Result::Ok(v1) => println(v1),
        Result::Err(_) => println(-1),
    }
    // 非法 i64（非数字）
    match json::try_parse::<i64>(String::from("abc")) {
        Result::Ok(_) => println(0),
        Result::Err(_) => println(-1),
    }
    // 部分合法 i64（"12x" 之前 string_to_int 宽松解析为 12，现应 Err）
    match json::try_parse::<i64>(String::from("12x")) {
        Result::Ok(_) => println(0),
        Result::Err(_) => println(-1),
    }
    // 合法 bool
    match json::try_parse::<bool>(String::from("false")) {
        Result::Ok(b1) => {
            if b1 { println(1) } else { println(0) }
        },
        Result::Err(_) => println(-1),
    }
    // 非法 bool
    match json::try_parse::<bool>(String::from("abc")) {
        Result::Ok(_) => println(0),
        Result::Err(_) => println(-1),
    }
    // 合法 String（带引号）
    match json::try_parse::<String>(String::from("\"hi\"")) {
        Result::Ok(s1) => println(s1),
        Result::Err(_) => println(-1),
    }
    // 未闭合 String
    match json::try_parse::<String>(String::from("\"hi")) {
        Result::Ok(_) => println(0),
        Result::Err(_) => println(-1),
    }
}
