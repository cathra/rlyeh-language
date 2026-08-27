// X3：JsonError/TomlError 错误类型 + Result<T, Error> 返回（错误处理基础）
// std serde/module.rl 定义 `enum JsonError { ParseError(String), InvalidType }`
// 与 `enum TomlError`；函数可返回 `Result<i64, JsonError>`（match 解包）。
// 经 `serde::` 路径访问（std serde 模块命名空间）。

fn try_parse_int(s: String) -> Result<i64, serde::JsonError> {
    if s.len == 0 {
        return Result::Err(serde::JsonError::ParseError(String::from("empty")));
    }
    Result::Ok(string_to_int(s))
}

fn main() -> i64 {
    // 正常解析
    let r1 = try_parse_int(String::from("42"));
    let mut total = match r1 {
        Result::Ok(v) => v,
        Result::Err(_) => -1,
    };

    // 错误路径（空输入 → ParseError）
    let r2 = try_parse_int(String::from(""));
    total = total + match r2 {
        Result::Ok(v) => v,
        Result::Err(_) => 100,
    };

    // TomlError 类型可用
    let te = serde::TomlError::InvalidType;
    total = total + match te {
        serde::TomlError::ParseError(_) => 1,
        serde::TomlError::InvalidType => 7,
    };

    total
}
