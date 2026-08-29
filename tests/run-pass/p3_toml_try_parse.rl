// P3（2026-08-28）：`toml::try_parse::<T>(s) -> Result<T, serde::TomlError>` 严格解析。
// 合法输入返回 Ok，非法输入返回 Err(TomlError::ParseError)。覆盖 i64 / bool / Vec 的
// 合法与非法输入（复用 core.rl `parse_int_strict` / `json_unescape_checked` 严格 helper）。

fn main() {
    // 合法 i64
    match toml::try_parse::<i64>(String::from("42")) {
        Result::Ok(v1) => println(v1),
        Result::Err(_) => println(-1),
    }
    // 非法 i64
    match toml::try_parse::<i64>(String::from("abc")) {
        Result::Ok(_) => println(0),
        Result::Err(_) => println(-1),
    }
    // 合法 bool
    match toml::try_parse::<bool>(String::from("true")) {
        Result::Ok(b1) => {
            if b1 { println(1) } else { println(0) }
        },
        Result::Err(_) => println(-1),
    }
    // 非法 bool
    match toml::try_parse::<bool>(String::from("abc")) {
        Result::Ok(_) => println(0),
        Result::Err(_) => println(-1),
    }
    // 合法 Vec
    match toml::try_parse::<Vec<i64>>(String::from("[1,2,3]")) {
        Result::Ok(vv) => println(vv.len()),
        Result::Err(_) => println(-1),
    }
    // 非法 Vec（未闭合 [）
    match toml::try_parse::<Vec<i64>>(String::from("[1,2,3")) {
        Result::Ok(_) => println(0),
        Result::Err(_) => println(-1),
    }
}
