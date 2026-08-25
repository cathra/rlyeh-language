// K1 Result `?` 解包：失败臂 Err(__e) => return Err(__e)（编译通过，不运行）
fn parse_num(s: String) -> Result<i64, String> {
    if s.len() == 0 {
        return Err(String::from("empty"));
    }
    Ok(s.len())
}

fn double_result(s: String) -> Result<i64, String> {
    let n = parse_num(s.clone())?;   // Result ? 解包
    let m = parse_num(s)?;           // 再解包
    Ok(n + m)
}

fn main() {
    let r1 = parse_num(String::from("abc"));
    match r1 {
        Ok(v) => println(v),
        Err(e) => println(0),
    }
    let r2 = double_result(String::from("x"));
    match r2 {
        Ok(v) => println(v),
        Err(e) => println(-1),
    }
    let r3 = parse_num(String::from(""));
    match r3 {
        Ok(v) => println(v),
        Err(e) => println(-2),
    }
}
